use clap::{Args, Parser};
use log::{warn, info};
use log::LevelFilter;
use anyhow::Result;
use nanohttp::{Method, Response, Status};
use serde::Serialize;
use indexmap::IndexMap;
use std::{error::Error, fmt::Display, fs, path::PathBuf, str::FromStr, net::SocketAddr};

#[derive(Clone, clap::ValueEnum, Default, Debug)]
pub enum LogType {
    Info,
    Warn,
    #[default]
    Error,
    Debug,
}

impl From<LogType> for LevelFilter {
    fn from(val: LogType) -> Self  {
        match val {
            LogType::Info => LevelFilter::Info,
            LogType::Warn => LevelFilter::Warn,
            LogType::Error => LevelFilter::Error,
            LogType::Debug => LevelFilter::Debug,
        }
    }
}

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
    pub allow_remote: bool,
    
    #[command(flatten)]
    pub log: Log
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

#[derive(Args, Debug)]
pub struct Log {
    /// Turn logging on
    #[arg(short, long, default_value_t=false)]
    pub log: bool,
    #[arg(long, default_value="./testsuite.log")]
    pub log_file: PathBuf,
    #[arg(long, default_value_t, value_enum)]
    pub log_filter: LogType
}

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
pub enum ResponseFormat {
    #[default]
    Json,
    Html,
    None,
}

#[derive(Debug)]
pub enum ConnectionFailedError {
    Connection(anyhow::Error),
    Parsing((SocketAddr, anyhow::Error)),
}

#[derive(Debug)]
pub enum Message {
    ConnectionFailed(ConnectionFailedError),
    ConnectionReceived(Option<SocketAddr>),
    Response(ResponseMessage),
}

#[derive(Debug)]
pub struct ResponseMessage {
    pub addr: SocketAddr,
    pub response: Response,
    pub method: Option<Method>,
    pub status: Option<Status>
}

impl ResponseMessage {
    pub fn new(addr: SocketAddr, response: &Response, method:Option<Method>, status:Option<Status>) -> Self {
        ResponseMessage {
            status,
            method,
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
            "" => Ok(ResponseFormat::None),
            _ => Err(ResponseFormatError::ParseFailedError(s.to_owned())),
        }
    }
}

#[derive(Default,Clone, Debug)]
pub struct EndpointContent {
    pub content: Option<String>,
    pub format: String,
}

impl ToString for EndpointContent {
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
impl EndpointContent {
    /// Creates a response from text content, endpoint, and format
    pub fn from_content(content: &str, response_format: &ResponseFormat) -> EndpointContent {
        EndpointContent {
            content: Some(String::from(content)),
            format: response_format.to_string(),
        }
    }

    /// Creates a response from content file and response format
    pub fn from_content_file(path: &PathBuf, response_format: &ResponseFormat) -> EndpointContent {
        EndpointContent {
            content: match path.exists() {
                true => Some(fs::read_to_string(path).expect("File is unreadable")),
                false => {
                    eprintln!("{}", format_args!("Could not find file: {:}, continuing with blank response", path.to_str().expect("Path is unparseable")));
                    None
                }
            },
            format: response_format.to_string(),
        }
    }

    /// Creates a response from Argument object
    pub fn from_args(args: &Arguments) -> EndpointContent {
        let response_format = &args.format;
        if let Some(content) = &args.content.content {
            EndpointContent::from_content(content, response_format)
        } else if let Some(p) = &args.content.content_file {
            EndpointContent::from_content_file(p, response_format)
        } else {
            EndpointContent {
                content: None,
                format: response_format.to_string(),
            }
        }
    }

    pub fn from_folder(
        path: &PathBuf,
    ) -> Result<IndexMap<String, EndpointContent>, FolderError> {
        if path.exists() {
            if path.is_dir() {
                Ok(fs::read_dir(path).map_err(|e| 
                        FolderError {
                            error: format!("{}", e.to_string())
                        })
                    ?.filter_map(|file| match file {
                        Ok(some) => {
                            Some(some)
                        },
                        Err(e) => {
                            warn!("{}", format!("Error: could not read directory: {:?}", e));
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
                                    info!("{}", format!("File: {} does not have a valid extension [html, json]", file.path().to_str()?));
                                    None
                                },
                                (Some(_), None) => {
                                    info!("{}", format!("File: {} does not have a file name for use in endpoint generation", file.path().to_str()?));
                                    None
                                },
                                _ => {
                                    None
                                }
                            }
                        } else {
                            info!("{}", format!("File: {} does not have an extension, valid extensions are [html, json]", file.path().to_str()?));
                            None
                        }

                    })
                    .map(|(format, stem)| {
                        let mut endpoint = String::from("/");
                        endpoint.push_str(stem.file_stem().unwrap().to_str().unwrap());
                        (endpoint, 
                            EndpointContent::from_content_file(
                                &stem,
                                &format,
                            ))
                    })
                    .collect())
            } else {
                Err(FolderError {error: "Path is not a directory".to_string()})
            }
        } else {
            Err(FolderError {error: "Path does not exists".to_string()})
        }
    }
}

pub fn populate_map(args: &Arguments) -> IndexMap<String, EndpointContent> {
    let mut map: IndexMap<String, EndpointContent> = IndexMap::new();
    let (content, content_file, content_folder) = (
        &args.content.content,
        &args.content.content_file,
        &args.content.content_folder,
    );
    match (content, content_file, content_folder) {
        (Some(content), None, None) => {
            map.insert(
                args.endpoint.clone(),
                EndpointContent::from_content(content, &args.format),
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
            map.insert(endpoint, EndpointContent::from_content_file(content_file, &args.format));
        }
        (None, None, Some(content_folder)) => {
            match EndpointContent::from_folder(content_folder) {
                Ok(map_b) => {
                    eprintln!("Valid endpoints: {:?}", map_b.keys().collect::<Vec<_>>());
                    map.extend(map_b)
                }
                Err(e) => {
                    warn!("Error while parsing content folder: {:?}", e);
                }
            }
        }
        _ => {
            map.insert(String::from("/"), EndpointContent::default());
        }
    }
    map
}
