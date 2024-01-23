use clap::{Args, Parser};
use anyhow::Result;
use colored::Colorize;
use nanohttp::{Response as HttpResponse, Method, Status};
use serde::Serialize;
use std::{collections::HashMap, error::Error, fmt::Display, fs, path::PathBuf, str::FromStr, net::SocketAddr};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Port to run on
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    #[command(flatten)]
    pub content: Content,

    /// Response format
    #[arg(short, long, default_value_t, value_enum)]
    pub format: ResponseFormat,

    #[arg(short, long, default_value_t = String::from("/"))]
    pub endpoint: String,

    #[arg(short, long, default_value_t = false)]
    pub allow_remote: bool
}

#[derive(Args, Debug)]
#[group(multiple = false)]
pub struct Content {
    /// Response content
    #[arg(short, long)]
    pub content: Option<String>,

    /// Response content file
    #[arg(long)]
    pub content_file: Option<PathBuf>,

    #[arg(long)]
    pub content_folder: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
pub enum ResponseFormat {
    #[default]
    Json,
    Html,
    None,
}

#[derive(Debug)]
pub enum Message {
    ConnectionFailed,
    ConnectionReceived(Option<SocketAddr>),
    Response(ResponseMessage),
}

#[derive(Debug)]
pub struct ResponseMessage {
    pub addr: SocketAddr,
    pub response: HttpResponse,
}

impl ResponseMessage {
    pub fn new(addr: SocketAddr, response: &HttpResponse) -> Self {
        ResponseMessage {
            addr,
            response: response.clone()
        }
    }
}


impl ToString for ResponseFormat {
    fn to_string(&self) -> String {
        match *self {
            ResponseFormat::Json => {
                "application/json".to_string()
            },
            ResponseFormat::Html => {
                "text/html".to_string()
            },
            ResponseFormat::None => {
                String::new()
            }
        }
    }
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

#[derive(Default,Clone, Debug)]
pub struct ResponseContent {
    pub content: Option<String>,
    pub format: ResponseFormat,
}

impl ToString for ResponseContent {
    /// Turns response into http response string
    fn to_string(&self) -> String {
        match &self.content {
            Some(content) => {
                content.to_string()
            },
            None => "".to_string()
        }
    }
}
#[derive(Debug, PartialEq, Clone)]
pub struct FolderError {
    error: String,
}
impl Display for FolderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(write!(f, "{:?}", self.error))?
    }
}
impl Error for FolderError {}
impl ResponseContent {
    /// Creates a response from text content, endpoint, and format
    pub fn from_content(content: &str, response_format: &ResponseFormat) -> ResponseContent {
        ResponseContent {
            content: Some(String::from(content)),
            format: response_format.clone(),
        }
    }

    /// Creates a response from content file and response format
    pub fn from_content_file(path: &PathBuf, response_format: &ResponseFormat) -> ResponseContent {
        ResponseContent {
            content: match path.exists() {
                true => Some(fs::read_to_string(path).expect("File is unreadable")),
                false => {
                    eprintln!("{}", format!("Could not find file: {:}, continuing with blank response", path.to_str().expect("Path is unparseable")).yellow());
                    None
                }
            },
            format: response_format.clone(),
        }
    }

    /// Creates a response from Argument object
    pub fn from_args(args: &Arguments) -> ResponseContent {
        let response_format = &args.format;
        if let Some(content) = &args.content.content {
            return ResponseContent::from_content(content, response_format);
        } else if let Some(p) = &args.content.content_file {
            return ResponseContent::from_content_file(p, response_format);
        } else {
            return ResponseContent {
                content: None,
                format: response_format.clone(),
            };
        }
    }

    pub fn from_folder(
        path: &PathBuf,
    ) -> Result<HashMap<String, ResponseContent>, FolderError> {
        if path.exists() {
            if path.is_dir() {
                let paths = fs::read_dir(path).map_err(|e| FolderError {
                    error: format!("{}", e.to_string()),
                })?;
                let map:HashMap<String, ResponseContent> = paths
                    .filter_map(|file| match file {
                        Ok(some) => {
                            Some(some)
                        },
                        Err(e) => {
                            eprintln!("{}", format!("Error: could not read directory: {:?}", e).red());
                            None
                        }
                    }).filter_map(|file| {
                        match file.path().is_dir() {
                            true => None,
                            false => {
                                Some(file)
                            }
                        }
                    })
                    .filter_map(|file| {
                        if let Some(ext) = file.path().extension() {
                            match (ext.to_str(), file.path().file_stem()) {
                                (Some("json"), Some(_)) => {
                                   Some((ResponseFormat::Json, file.path()))
                                },
                                (Some("html"), Some(_)) => {
                                    Some((ResponseFormat::Html, file.path()))
                                },
                                (_, Some(_)) => {
                                    eprintln!("{}", format!("File: {} does not have a valid extension [html, json]", file.path().to_str()?).yellow());
                                    None
                                },
                                (Some(_), None) => {
                                    eprintln!("{}", format!("File: {} does not have a file name for use in endpoint generation", file.path().to_str()?).yellow());
                                    None
                                },
                                _ => {
                                    None
                                }
                            }
                        } else {
                            eprintln!("{}", format!("File: {} does not have an extension, valid extensions are [html, json]", file.path().to_str()?).yellow());
                            None
                        }

                    })
                    .map(|(format, stem)| {
                        let mut endpoint = String::from("/");
                        endpoint.push_str(stem.file_stem().unwrap().to_str().unwrap());
                        (endpoint, 
                            ResponseContent::from_content_file(
                                &stem,
                                &format,
                            ))
                    })
                    .collect();

                return Ok(map);
            } else {
                return Err(FolderError {error: "Path is not a directory".to_string()});
            }
        } else {
            return Err(FolderError {error: "Path does not exists".to_string()});
        }
    }
}

pub fn populate_map(args: &Arguments) -> HashMap<String, ResponseContent> {
    let mut map: HashMap<String, ResponseContent> = HashMap::new();
    let (content, content_file, content_folder) = (
        &args.content.content,
        &args.content.content_file,
        &args.content.content_folder,
    );
    match (content, content_file, content_folder) {
        (Some(content), None, None) => {
            map.insert(
                args.endpoint.clone(),
                ResponseContent::from_content(&content, &args.format),
            );
        }
        (None, Some(content_file), None) => {
            let endpoint = match content_file.file_stem() {
                Some(path) => {
                    let mut endpoint: String = String::from("/");
                    endpoint.push_str(&path.to_string_lossy());
                    endpoint
                }
                None => String::from("/"),
            };
            map.insert(endpoint, ResponseContent::from_content_file(&content_file, &args.format));
        }
        (None, None, Some(content_folder)) => {
            match ResponseContent::from_folder(&content_folder) {
                Ok(map_b) => {
                    eprintln!("Valid endpoints: {:?}", map_b.keys().collect::<Vec<_>>());
                    map.extend(map_b)
                }
                Err(e) => {
                    eprintln!("Error while parsing content folder: {:?}", e);
                }
            }
        }
        _ => {
            map.insert(String::from("/"), ResponseContent::default());
        }
    }
    map
}

