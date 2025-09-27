//! Multi-model agent with fallback support
//! 
//! This agent supports multiple model providers with automatic fallback:
//! - Primary: OpenRouter (from constructor parameters)
//! - Secondary: Vercel AI Gateway (from VERCEL_AI_GATEWAY_API_KEY and VERCEL_AI_GATEWAY_MODEL env vars)
//! 
//! If one provider returns a non-200 response, the agent automatically tries the next one
//! until all providers are exhausted.

use crate::agent::{Agent, AgentError, AgentInfo, AgentResponse, ResponseMetadata};
use crate::events::{AppEvent, EventSender, ToolName, TokenUsage};
use crate::session::ChatMessage;
use crate::tools::{ToolExecutor, ToolRegistry};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub name: String,
}

pub struct MultiModelAgent {
    info: AgentInfo,
    model_configs: Vec<ModelConfig>,
    event_sender: EventSender,
    tools: ToolRegistry,
}

impl MultiModelAgent {
    pub fn new(
        api_key: String,
        model: String,
        event_sender: EventSender,
    ) -> anyhow::Result<Self> {
        // Build model configurations with fallback support
        let mut model_configs = Vec::new();
        
        // Primary OpenRouter config
        model_configs.push(ModelConfig {
            base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            api_key: api_key.clone(),
            model: model.clone(),
            name: "OpenRouter".to_string(),
        });
        
        // Vercel AI Gateway config (if available)
        if let Ok(vercel_api_key) = std::env::var("VERCEL_AI_GATEWAY_API_KEY") {
            if let Ok(vercel_model) = std::env::var("VERCEL_AI_GATEWAY_MODEL") {
                model_configs.push(ModelConfig {
                    base_url: "https://ai-gateway.vercel.sh/v1/chat/completions".to_string(),
                    api_key: vercel_api_key,
                    model: vercel_model,
                    name: "Vercel AI Gateway".to_string(),
                });
            }
        }
        
        // Fallback to original params if no additional configs
        if model_configs.len() == 1 {
            model_configs.push(ModelConfig {
                base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
                api_key,
                model,
                name: "OpenRouter Fallback".to_string(),
            });
        }
        
        Ok(Self {
            info: AgentInfo {
                name: "Multi-Model Agent".to_string(),
                description: "Agent with multiple model provider support and fallback".to_string(),
                version: "0.1.0".to_string(),
            },
            model_configs,
            event_sender,
            tools: ToolRegistry::new(),
        })
    }

    fn tool_name_from_string(&self, name: &str) -> Option<ToolName> {
        match name {
            "fs.read" => Some(ToolName::FsRead),
            "fs.search" => Some(ToolName::FsSearch),
            "fs.write" => Some(ToolName::FsWrite),
            "fs.apply_patch" => Some(ToolName::FsApplyPatch),
            "fs.set_file" => Some(ToolName::FsSetFile),
            "fs.replace_once" => Some(ToolName::FsReplaceOnce),
            "fs.insert_before" => Some(ToolName::FsInsertBefore),
            "fs.insert_after" => Some(ToolName::FsInsertAfter),
            "fs.delete_file" => Some(ToolName::FsDeleteFile),
            "fs.rename_file" => Some(ToolName::FsRenameFile),
            "fs.find" => Some(ToolName::FsFind),
            "shell.exec" => Some(ToolName::ShellExec),
            "code.symbols" => Some(ToolName::CodeSymbols),
            "large_context_fetch" => Some(ToolName::LargeContextFetch),
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
                    ToolName::FsSetFile => "fs.set_file",
                    ToolName::FsReplaceOnce => "fs.replace_once",
                    ToolName::FsInsertBefore => "fs.insert_before",
                    ToolName::FsInsertAfter => "fs.insert_after",
                    ToolName::FsDeleteFile => "fs.delete_file",
                    ToolName::FsRenameFile => "fs.rename_file",
                    ToolName::FsFind => "fs.find",
                    ToolName::ShellExec => "shell.exec",
                    ToolName::CodeSymbols => "code.symbols",
                    ToolName::LargeContextFetch => "large_context_fetch",
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

    fn get_system_prompt(&self) -> String {
        include_str!("../prompts/system_prompt.md").to_string()
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
                    crate::session::MessageRole::Tool => "tool",
                };
                let content = match m.role {
                    crate::session::MessageRole::Error => format!("[error] {}", m.content),
                    crate::session::MessageRole::Tool => {
                        // For tool messages, we need to format them as tool responses
                        if let Some(ref tool_info) = m.tool_info {
                            // Combine result, stdout, and stderr into a single JSON payload
                            let combined = json!({
                                // "result": tool_info.result.clone().unwrap_or(json!(null)),
                                "stdout": tool_info.stdout,
                                "stderr": tool_info.stderr,
                            });
                            serde_json::to_string(&combined).unwrap_or_else(|_| "{}".to_string())
                        } else {
                            m.content.clone()
                        }
                    },
                    _ => m.content.clone(),
                };
                
                if m.role == crate::session::MessageRole::Tool {
                    // Tool messages need special formatting for OpenAI API
                    if let Some(ref tool_info) = m.tool_info {
                        json!({
                            "role": role,
                            "tool_call_id": tool_info.id,
                            "content": content
                        })
                    } else {
                        json!({"role": role, "content": content})
                    }
                } else {
                    json!({"role": role, "content": content})
                }
            })
            .collect()
    }

    async fn http_post(&self, body: &Value) -> Result<ChatCompletionResponse, AgentError> {
        let client = reqwest::Client::new();
        let mut last_error = None;
        
        // Try each model config until one succeeds
        for (i, config) in self.model_configs.iter().enumerate() {
            // Update the body with the current config's model
            let mut request_body = body.clone();
            if let Some(model_obj) = request_body.get_mut("model") {
                *model_obj = json!(config.model);
            }
            
            let req = client
                .post(&config.base_url)
                .bearer_auth(&config.api_key)
                .header("Content-Type", "application/json");

            let resp = match req.json(&request_body).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    let error_msg = format!("{} request error: {}", config.name, e);
                    last_error = Some(error_msg.clone());
                    
                    // Log the error but continue to next config
                    let _ = self.event_sender.send(AppEvent::Error { 
                        id: None, 
                        message: format!("Failed to connect to {}, trying next provider...", config.name)
                    });
                    continue;
                }
            };

            if resp.status().is_success() {
                match resp.json::<ChatCompletionResponse>().await {
                    Ok(parsed) => {
                        // Success! Log which provider was used
                        if i > 0 {
                            let _ = self.event_sender.send(AppEvent::Error { 
                                id: None, 
                                message: format!("Successfully using {} after {} failed attempts", config.name, i)
                            });
                        }
                        return Ok(parsed);
                    }
                    Err(e) => {
                        let error_msg = format!("{} decode error: {}", config.name, e);
                        last_error = Some(error_msg);
                        continue;
                    }
                }
            } else {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                let error_msg = format!("{} HTTP {}: {}", config.name, status, text);
                last_error = Some(error_msg.clone());
                
                // Log non-success status but continue to next config
                let _ = self.event_sender.send(AppEvent::Error { 
                    id: None, 
                    message: format!("{} returned {}, trying next provider...", config.name, status)
                });
                continue;
            }
        }
        
        // All configs failed
        Err(AgentError::Network(
            last_error.unwrap_or_else(|| "All model providers failed".to_string())
        ))
    }
}

#[async_trait]
impl Agent for MultiModelAgent {
    async fn submit(
        &self,
        message: String,
        history: Vec<ChatMessage>,
    ) -> Result<AgentResponse, AgentError> {
        let start = Instant::now();

        // Seed with system prompt, history, and current user message
        let mut messages = vec![json!({
            "role": "system",
            "content": self.get_system_prompt()
        })];
        messages.extend(self.convert_history(&history));
        messages.push(json!({ "role": "user", "content": message }));

        let tools = self.tool_specs_for_openai();
        let mut turns = 0usize;
        let mut final_text = String::new();
        let mut token_usage: Option<TokenUsage> = None;

        loop {
            turns += 1;

            let body = json!({
                "model": self.model_configs[0].model, // Will be updated in http_post for each config
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
                    // Add the assistant's message with tool calls to the conversation
                    messages.push(json!({
                        "role": "assistant",
                        "content": msg.content,
                        "tool_calls": tool_calls
                    }));

                    let executor = ToolExecutor::new(self.event_sender.clone())
                        .with_max_output_size(1024 * 1024); // 1MB limit, can be overridden by GROK_TOOL_MAX_OUTPUT_SIZE env var
                    
                    for call in tool_calls {
                        let name = call.function.name;
                        let tool_name = self.tool_name_from_string(&name)
                            .ok_or_else(|| AgentError::Processing(format!("unknown tool: {}", name)))?;
                        let args: Value = serde_json::from_str(&call.function.arguments)
                            .map_err(|e| AgentError::Processing(format!("invalid tool args: {}", e)))?;

                        if let Err(e) = self.tools.validate_args(&tool_name, &args) {
                            let _ = self.event_sender.send(AppEvent::Error { id: None, message: format!("tool args validation failed: {}", e) });
                            continue;
                        }

                        // Execute tool and get result
                        let tool_result = match executor.execute_tool_with_result(call.id.clone(), tool_name.clone(), args.clone()).await {
                            Ok(result) => result,
                            Err(e) => {
                                // Return error as JSON string for the LLM to understand
                                json!({
                                    "error": e.to_string(),
                                    "tool": format!("{:?}", tool_name),
                                    "args": args
                                })
                            }
                        };

                        // Add tool result to conversation following OpenRouter format
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": call.id,
                            "content": serde_json::to_string(&tool_result).unwrap_or_else(|_| "{}".to_string())
                        }));
                    }
                    
                    // Continue loop for next assistant turn
                    continue;
                }

                // Assistant content present, finish
                if let Some(content) = msg.content {
                    // Add the assistant's final response to the conversation
                    messages.push(json!({
                        "role": "assistant",
                        "content": content
                    }));
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
struct ChatCompletionResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    model: String,
    #[serde(default)]
    usage: Option<TokenUsageResponse>,
    choices: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenUsageResponse { prompt_tokens: i64, completion_tokens: i64, total_tokens: i64 }

#[derive(Debug, Clone, Deserialize)]
struct Choice {
    #[allow(dead_code)]
    finish_reason: Option<String>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Clone, Deserialize)]
struct Message {
    #[allow(dead_code)]
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>, 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionCall { name: String, arguments: String }


