// Fin — SSE Stream Parsing Utilities
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

/// Parse a Server-Sent Events (SSE) line into event name and data.
///
/// SSE format:
/// ```text
/// event: event_name
/// data: {"json": "payload"}
/// ```
#[allow(dead_code)]
pub fn parse_sse_line(line: &str) -> Option<SseLine> {
    if let Some(event) = line.strip_prefix("event: ") {
        Some(SseLine::Event(event.to_string()))
    } else if let Some(data) = line.strip_prefix("data: ") {
        if data == "[DONE]" {
            Some(SseLine::Done)
        } else {
            Some(SseLine::Data(data.to_string()))
        }
    } else if line.is_empty() {
        Some(SseLine::Empty)
    } else {
        None
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum SseLine {
    Event(String),
    Data(String),
    Done,
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_event_line() {
        let result = parse_sse_line("event: message_start");
        assert_eq!(result, Some(SseLine::Event("message_start".into())));
    }

    #[test]
    fn parse_data_line() {
        let result = parse_sse_line("data: {\"type\":\"content\"}");
        assert_eq!(result, Some(SseLine::Data("{\"type\":\"content\"}".into())));
    }

    #[test]
    fn parse_done_marker() {
        let result = parse_sse_line("data: [DONE]");
        assert_eq!(result, Some(SseLine::Done));
    }

    #[test]
    fn parse_empty_line() {
        let result = parse_sse_line("");
        assert_eq!(result, Some(SseLine::Empty));
    }

    #[test]
    fn parse_unknown_line() {
        assert_eq!(parse_sse_line("id: 123"), None);
        assert_eq!(parse_sse_line("retry: 5000"), None);
        assert_eq!(parse_sse_line(":comment"), None);
    }

    #[test]
    fn parse_data_with_spaces() {
        let result = parse_sse_line("data: hello world");
        assert_eq!(result, Some(SseLine::Data("hello world".into())));
    }

    #[test]
    fn parse_event_with_empty_value() {
        let result = parse_sse_line("event: ");
        assert_eq!(result, Some(SseLine::Event("".into())));
    }

    #[test]
    fn parse_data_with_empty_value() {
        let result = parse_sse_line("data: ");
        assert_eq!(result, Some(SseLine::Data("".into())));
    }
}
