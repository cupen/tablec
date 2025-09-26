use clap::Args;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use crate::cmd::build;
use crate::cmd::check;

#[derive(Args, Debug)]
pub struct WebCommand {
    #[arg(long)]
    pub listen: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BuildRequest {
    input: String,
    output: String,
    format: Option<String>,
    include_fields: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CheckRequest {
    path: String,
    verbose: Option<bool>,
}


struct AppState {
}

#[get("/")]
async fn index() -> Result<impl Responder> {
    let html = include_str!("../../static/index.html");
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

#[get("/api/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some("tablec web server is running".to_string()),
        message: None,
    })
}

#[post("/api/build")]
async fn build_endpoint(
    req: web::Json<BuildRequest>,
    _data: web::Data<Mutex<AppState>>,
) -> Result<impl Responder> {
    let format = req.format.as_deref().unwrap_or("json");
    let include_fields = req.include_fields.unwrap_or(false);
    
    match build::build_to_string(&req.input, format, include_fields) {
        Ok(result) => {
            let output_path = &req.output;
            if let Err(e) = std::fs::write(output_path, &result) {
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<String> {
                    success: false,
                    data: None,
                    message: Some(format!("Failed to write output: {}", e)),
                }));
            }
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(format!("Successfully built tables to {}", 
                    output_path)),
                message: None,
            }))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            data: None,
            message: Some(format!("Build failed: {}", e)),
        })),
    }
}

#[post("/api/check")]
async fn check_endpoint(
    req: web::Json<CheckRequest>,
) -> Result<impl Responder> {
    let verbose = req.verbose.unwrap_or(false);
    
    let check_cmd = check::CheckCommand {
        verbose,
        path: Some(std::path::PathBuf::from(&req.path)),
    };
    
    match check_cmd.run() {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some("All tables validated successfully".to_string()),
            message: None,
        })),
        Err(e) => Ok(HttpResponse::Ok().json(ApiResponse::<String> {
            success: false,
            data: None,
            message: Some(format!("Validation failed: {}", e)),
        })),
    }
}

#[get("/api/formats")]
async fn supported_formats() -> impl Responder {
    let formats = vec!["json".to_string(), "msgpack".to_string(), "protobuf".to_string()];
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(formats),
        message: None,
    })
}

impl WebCommand {
    pub async fn run(&self) -> std::io::Result<()> {
        return _run(self).await;
    }
}

async fn _run(command: &WebCommand) -> std::io::Result<()> {
    println!("Starting tablec web server on: {}", command.listen);
    
    let app_state = web::Data::new(Mutex::new(AppState {
    }));
    
    // Create uploads directory if it doesn't exist
    let _ = std::fs::create_dir_all("./uploads");
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(index)
            .service(health)
            .service(build_endpoint)
            .service(check_endpoint)
            .service(supported_formats)
    })
    .bind(&command.listen)?
    .run()
    .await
}