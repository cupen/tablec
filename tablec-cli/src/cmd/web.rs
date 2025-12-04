use actix_web::{get, App, HttpResponse, HttpServer, Responder};
use clap::Args;
use std::error::Error;

#[derive(Args, Debug)]
pub struct WebCommand {
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub listen: String,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

impl WebCommand {
    pub async fn run(self) -> Result<(), Box<dyn Error>> {
        println!("Starting web server on {}", self.listen);

        HttpServer::new(|| {
            App::new()
                .service(hello)
                .service(health)
        })
        .bind(self.listen)?
        .run()
        .await?;

        Ok(())
    }
}