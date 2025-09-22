use crate::agent::{Agent, AgentError, AgentInfo, AgentResponse, ResponseMetadata};
use crate::events::{AppEvent, EventSender, ToolName, TokenUsage};
use crate::session::ChatMessage;
use crate::tools::{ToolExecutor, ToolRegistry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenRouterAgent {
    info: AgentInfo,
    api_key: String,
    model: String,
    referer: Option<String>,
    title: Option<String>,
    event_sender: EventSender,
    tools: ToolRegistry,
}

impl OpenRouterAgent {
    pub fn new(
        api_key: String,
        model: String,
        referer: Option<String>,
        title: Option<String>,
        event_sender: EventSender,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            info: AgentInfo {
                name: "OpenRouter Agent".to_string(),
                description: "Agent powered by OpenRouter with tool calling".to_string(),
                version: "0.1.0".to_string(),
            },
            api_key,
            model,
            referer,
            title,
            event_sender,
            tools: ToolRegistry::new(),
        })
    }

    fn map_tool_name(&self, name: &str) -> Option<ToolName> {
        match name {
            "fs.read" => Some(ToolName::FsRead),
            "fs.search" => Some(ToolName::FsSearch),
            "fs.write" => Some(ToolName::FsWrite),
            "fs.apply_patch" => Some(ToolName::FsApplyPatch),
            "shell.exec" => Some(ToolName::ShellExec),
            _ => None,
        }
    }

    fn tool_specs_for_openai(&self) -> Vec<Value> {
        self.tools
            .get_all_specs()
            .into_iter()
            .map(|spec| {
                let name = match spec.name {
                    ToolName::FsRead => "fs.read",
                    ToolName::FsSearch => "fs.search",
                    ToolName::FsWrite => "fs.write",
                    ToolName::FsApplyPatch => "fs.apply_patch",
                    ToolName::ShellExec => "shell.exec",
                };
                json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": format!("Tool: {:?}", spec.name),
                        "parameters": spec.input_schema,
                    }
                })
            })
            .collect()
    }

    fn convert_history(&self, history: &[ChatMessage]) -> Vec<Value> {
        history
            .iter()
            .map(|m| {
                let role = match m.role {
                    crate::session::MessageRole::User => "user",
                    crate::session::MessageRole::Agent => "assistant",
                    crate::session::MessageRole::System => "system",
                    crate::session::MessageRole::Error => "system",
                };
                let content = match m.role {
                    crate::session::MessageRole::Error => format!("[error] {}", m.content),
                    _ => m.content.clone(),
                };
                json!({"role": role, "content": content})
            })
            .collect()
    }

    async fn http_post(&self, body: &Value) -> Result<OpenRouterResponse, AgentError> {
        let client = reqwest::Client::new();
        let mut req = client
            .post(OPENROUTER_URL)
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json");
        if let Some(ref r) = self.referer { req = req.header("HTTP-Referer", r); }
        if let Some(ref t) = self.title { req = req.header("X-Title", t); }

        let resp = req
            .json(body)
            .send()
            .await
            .map_err(|e| AgentError::Network(format!("request error: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AgentError::Network(format!("{}: {}", status, text)));
        }

        let parsed: OpenRouterResponse = resp
            .json()
            .await
            .map_err(|e| AgentError::Network(format!("decode error: {}", e)))?;
        Ok(parsed)
    }
}

#[async_trait]
impl Agent for OpenRouterAgent {
    async fn submit(
        &self,
        message: String,
        mut history: Vec<ChatMessage>,
    ) -> Result<AgentResponse, AgentError> {
        let start = Instant::now();

        // Seed with history and current user message
        let mut messages = self.convert_history(&history);
        messages.push(json!({ "role": "user", "content": message }));

        let tools = self.tool_specs_for_openai();
        let mut turns = 0usize;
        let mut final_text = String::new();
        let mut token_usage: Option<TokenUsage> = None;

        loop {
            if turns > 8 { return Err(AgentError::Processing("Too many tool turns".to_string())); }
            turns += 1;

            let body = json!({
                "model": self.model,
                "messages": messages,
                "tools": tools,
                "tool_choice": "auto"
            });

            // First turn event
            if turns == 1 { let _ = self.event_sender.send(AppEvent::ChatCreated); }

            let resp = self.http_post(&body).await?;

            if let Some(usage) = resp.usage.clone() {
                token_usage = Some(TokenUsage {
                    input_tokens: usage.prompt_tokens as u32,
                    output_tokens: usage.completion_tokens as u32,
                    total_tokens: usage.total_tokens as u32,
                });
            }

            let Some(choice) = resp.choices.into_iter().next() else {
                return Err(AgentError::Processing("no choices".to_string()));
            };

            // Tool calls?
            if let Some(msg) = choice.message {
                if let Some(tool_calls) = msg.tool_calls {
                    let executor = ToolExecutor::new(self.event_sender.clone());
                    for call in tool_calls {
                        let name = call.function.name;
                        let tool_name = self.map_tool_name(&name)
                            .ok_or_else(|| AgentError::Processing(format!("unknown tool: {}", name)))?;
                        let args: Value = serde_json::from_str(&call.function.arguments)
                            .map_err(|e| AgentError::Processing(format!("invalid tool args: {}", e)))?;

                        if let Err(e) = self.tools.validate_args(&tool_name, &args) {
                            let _ = self.event_sender.send(AppEvent::Error { id: None, message: format!("tool args validation failed: {}", e) });
                            continue;
                        }

                        // Execute tool
                        let _ = executor.execute_tool(call.id.clone(), tool_name.clone(), args.clone()).await;

                        // For transcript, echo back a tool message with an empty content (UI holds actual result in events)
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": call.id,
                            "content": "{\"ok\":true}"
                        }));
                    }
                    // Continue loop for next assistant turn
                    continue;
                }

                // Assistant content present, finish
                if let Some(content) = msg.content {
                    final_text = content;
                    break;
                }
            }

            // If we reach here without content or tools, stop
            break;
        }

        // Emit completion
        let _ = self.event_sender.send(AppEvent::ChatCompleted { token_usage: token_usage.clone() });
        if let Some(u) = token_usage.clone() { let _ = self.event_sender.send(AppEvent::TokenCount(u)); }

        Ok(AgentResponse {
            content: final_text,
            metadata: ResponseMetadata::new()
                .with_processing_time(start.elapsed()),
        })
    }

    fn info(&self) -> AgentInfo {
        self.info.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OpenRouterResponse {
    id: String,
    model: String,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
    choices: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenRouterUsage { prompt_tokens: i64, completion_tokens: i64, total_tokens: i64 }

#[derive(Debug, Clone, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    finish_reason: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Clone, Deserialize)]
struct Message {
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>, 
}

#[derive(Debug, Clone, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize)]
struct FunctionCall { name: String, arguments: String }


