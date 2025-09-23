use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Converts markdown text to styled ratatui Lines
pub fn parse_markdown(text: &str) -> Vec<Line<'static>> {
    let parser = Parser::new(text);
    let mut lines = Vec::new();
    let mut current_line: Vec<Span> = Vec::new();
    let mut style_stack = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();
    let mut list_depth: usize = 0;
    
    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading { .. } => {
                        style_stack.push(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
                    }
                    Tag::Emphasis => {
                        style_stack.push(Style::default().add_modifier(Modifier::ITALIC));
                    }
                    Tag::Strong => {
                        style_stack.push(Style::default().add_modifier(Modifier::BOLD));
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                        // End current line if there's content
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                        // Add a separator line before code block
                        lines.push(Line::from(Span::styled(
                            "┌─ Code Block ─────────────────────────────",
                            Style::default().fg(Color::DarkGray)
                        )));
                    }
                    Tag::List(_) => {
                        list_depth += 1;
                    }
                    Tag::Item => {
                        // End current line for list item
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                        // Add list marker
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        current_line.push(Span::styled(
                            format!("{}• ", indent),
                            Style::default().fg(Color::Cyan)
                        ));
                    }
                    Tag::Paragraph => {
                        // Start a new paragraph
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                    }
                    Tag::BlockQuote(_) => {
                        style_stack.push(Style::default().fg(Color::DarkGray));
                        current_line.push(Span::styled("│ ", Style::default().fg(Color::DarkGray)));
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    TagEnd::Heading(_) | TagEnd::Emphasis | TagEnd::Strong | TagEnd::BlockQuote => {
                        style_stack.pop();
                    }
                    TagEnd::CodeBlock => {
                        in_code_block = false;
                        // Add all code block lines with code styling
                        for code_line in &code_block_lines {
                            lines.push(Line::from(Span::styled(
                                format!("│ {}", code_line),
                                Style::default().fg(Color::Green).bg(Color::Black)
                            )));
                        }
                        code_block_lines.clear();
                        // Add closing border
                        lines.push(Line::from(Span::styled(
                            "└─────────────────────────────────────────",
                            Style::default().fg(Color::DarkGray)
                        )));
                        // Add a blank line after code block
                        lines.push(Line::from(""));
                    }
                    TagEnd::List(_) => {
                        list_depth = list_depth.saturating_sub(1);
                    }
                    TagEnd::Item => {
                        // End the list item line
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                    }
                    TagEnd::Paragraph => {
                        // End paragraph and add spacing
                        if !current_line.is_empty() {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                        lines.push(Line::from(""));
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    // Collect code block text
                    code_block_lines.extend(text.lines().map(|line| line.to_string()));
                } else {
                    let current_style = style_stack.last().copied().unwrap_or_default();
                    
                    // Handle line breaks in text
                    let text_lines: Vec<&str> = text.lines().collect();
                    for (i, line) in text_lines.iter().enumerate() {
                        if i > 0 {
                            // New line, push current line and start new one
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                        }
                        if !line.is_empty() {
                            current_line.push(Span::styled(line.to_string(), current_style));
                        }
                    }
                }
            }
            Event::Code(text) => {
                // Inline code
                current_line.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Green).bg(Color::Black)
                ));
            }
            Event::SoftBreak => {
                current_line.push(Span::raw(" "));
            }
            Event::HardBreak => {
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
            }
            Event::Rule => {
                // Horizontal rule
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }
                lines.push(Line::from(Span::styled(
                    "─".repeat(80),
                    Style::default().fg(Color::DarkGray)
                )));
                lines.push(Line::from(""));
            }
            _ => {}
        }
    }
    
    // Add any remaining content
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    
    // Remove trailing empty lines but keep at least one if the original had content
    while lines.len() > 1 && lines.last().map_or(false, |line| {
        line.spans.is_empty() || (line.spans.len() == 1 && line.spans[0].content.trim().is_empty())
    }) {
        lines.pop();
    }
    
    lines
}

/// Wraps markdown lines to fit within a given width
pub fn wrap_markdown_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    // If width is 0, don't wrap to avoid infinite loops
    if width == 0 {
        return lines;
    }

    let mut wrapped_lines = Vec::new();
    
    for line in lines {
        if line.spans.is_empty() {
            wrapped_lines.push(line);
            continue;
        }
        
        let total_content_len: usize = line.spans.iter()
            .map(|span| span.content.len())
            .sum();
            
        if total_content_len <= width {
            wrapped_lines.push(line);
        } else {
            // Need to wrap this line
            let mut current_line_spans: Vec<Span> = Vec::new();
            let mut current_line_len = 0;
            
            for span in line.spans {
                let words: Vec<&str> = span.content.split_whitespace().collect();
                let mut remaining_text = String::new();
                
                for (i, word) in words.iter().enumerate() {
                    if i > 0 {
                        remaining_text.push(' ');
                    }
                    remaining_text.push_str(word);
                }
                
                if remaining_text.is_empty() {
                    continue;
                }
                
                let words: Vec<&str> = remaining_text.split_whitespace().collect();
                let mut word_index = 0;
                
                while word_index < words.len() {
                    let word = words[word_index];
                    let word_len = word.len() + if current_line_len > 0 { 1 } else { 0 };
                    
                    if current_line_len + word_len <= width || current_line_spans.is_empty() {
                        // Add word to current line
                        if current_line_len > 0 {
                            if let Some(last_span) = current_line_spans.last_mut() {
                                if last_span.style == span.style {
                                    last_span.content = format!("{} {}", last_span.content, word).into();
                                } else {
                                    current_line_spans.push(Span::styled(format!(" {}", word), span.style));
                                }
                            } else {
                                current_line_spans.push(Span::styled(format!(" {}", word), span.style));
                            }
                        } else {
                            current_line_spans.push(Span::styled(word.to_string(), span.style));
                        }
                        current_line_len += word_len;
                        word_index += 1;
                    } else {
                        // Start new line
                        wrapped_lines.push(Line::from(current_line_spans.clone()));
                        current_line_spans.clear();
                        current_line_len = 0;
                    }
                }
            }
            
            if !current_line_spans.is_empty() {
                wrapped_lines.push(Line::from(current_line_spans));
            }
        }
    }
    
    wrapped_lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown_parsing() {
        let markdown = "# Hello World\n\nThis is **bold** text and *italic* text.\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\n- Item 1\n- Item 2";
        let lines = parse_markdown(markdown);
        
        // Should have multiple lines with different styles
        assert!(!lines.is_empty());
        
        // Check that we have some content
        let total_chars: usize = lines.iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.content.len())
            .sum();
        assert!(total_chars > 0);
    }

    #[test]
    fn test_wrapping() {
        let lines = vec![
            Line::from(vec![
                Span::raw("This is a very long line that should be wrapped when the width is too small for the content")
            ])
        ];
        
        let wrapped = wrap_markdown_lines(lines, 20);
        assert!(wrapped.len() > 1); // Should be wrapped into multiple lines
    }

    #[test]
    fn test_code_block_rendering() {
        let markdown = "Here's some code:\n\n```rust\nfn hello() {\n    println!(\"Hello, world!\");\n}\n```\n\nThat was code.";
        let lines = parse_markdown(markdown);
        
        // Should contain code block borders and content
        let content: String = lines.iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
            
        assert!(content.contains("Code Block"));
        assert!(content.contains("hello()"));
        assert!(content.contains("│")); // Code block should have borders
    }
}
