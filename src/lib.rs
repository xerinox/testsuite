use serde::Serialize;
use std::{fs, path::PathBuf, str::FromStr};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Port to run on
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Response format
    #[arg(short, long, default_value_t, value_enum)]
    pub format: ResponseFormat,

    /// Response content file
    #[arg(short = 'C', long, conflicts_with = "content")]
    pub content_file: Option<PathBuf>,

    /// Response content
    #[arg(short, long, conflicts_with = "content_file")]
    pub content: Option<String>,

    #[arg(short, long, default_value_t = String::from("/"))]
    pub endpoint: String
}

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
pub enum ResponseFormat {
    #[default]
    Json,
    Html,
}

#[derive(Debug)]
pub enum ResponseFormatError {
    ParseFailedError(String),
}

impl FromStr for ResponseFormat {
    type Err = ResponseFormatError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Ok(ResponseFormat::Json),
            "html" => Ok(ResponseFormat::Html),
            _ => Err(ResponseFormatError::ParseFailedError(s.to_owned())),
        }
    }
}

#[derive(Default)]
pub struct Response {
    pub content: Option<String>,
    pub format: ResponseFormat,
    pub endpoint: String
}

impl ToString for Response {
    fn to_string(&self) -> String {
        let mut response = String::from("HTTP/1.1 200 OK\r\n");
        match &self.format {
            &ResponseFormat::Html => {
                response.push_str("Content-Type: text/html; charset=utf-8\r\n");
            }
            &ResponseFormat::Json => {
                response.push_str("Content-Type: application/json\r\n");
            }
        }
        let content = match &self.content  {
            Some(content) => content,
            None => ""
        };
        response.push_str("Content-Length: ");
        response.push_str(&content.len().to_string());
        response.push_str("\r\n\r\n");
        response.push_str(&content);

        response
    }
}

impl Response {
    pub fn from_content(content: &str, endpoint: &str, response_format: &ResponseFormat) -> Response {
        Response {
            content: Some(String::from(content)),
            format: response_format.clone(),
            endpoint: String::from(endpoint)
        }
    }

    pub fn from_content_file(path: &PathBuf, response_format: &ResponseFormat) -> Response {
        Response {
            content: match path.exists() {
                true => Some(fs::read_to_string(path).expect("File is unreadable")),
                false => {
                    println!("Could not find file: {:}, continuing with blank response", path.to_str().expect("Path is unparseable"));
                    None
                },
            },
            format: response_format.clone(),
            endpoint: match path.file_stem() {
                Some(path) => {
                    let mut endpoint: String = String::from("/");
                        endpoint.push_str(&path.to_string_lossy());
                        endpoint
                    
                },
                None => String::from("/")
            },
        }
    }

    pub fn from_args(args: &Arguments) -> Response {
        let response_format = &args.format;
        if let Some(content) = &args.content {
            return Response::from_content(content, &args.endpoint, response_format)
        } else if let Some(p) = &args.content_file {
            return Response::from_content_file(p, response_format) 
        } else {
            return Response {
                content: None,
                format: response_format.clone(),
                endpoint: String::from("/")
            }
        }
    }
}
