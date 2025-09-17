use actix_cors::Cors;
use actix_web_prom::PrometheusMetricsBuilder;
use lazy_static::lazy_static;
use log::{error, info, LevelFilter};
use regex::Regex;
use serde::Deserialize;
use std::{collections::HashMap, error::Error, fs::OpenOptions, process::Command, str::FromStr};

use actix_web::{
    App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web::Json,
};
use declarative_env::declarative_env;

lazy_static! {
    static ref RFC1123_REGEX: Regex =
        Regex::new(r"^[a-z0-9][a-z0-9-]{0,61}[a-z0-9]?$").expect("failed to init RFC1123 regex");
    static ref SECRET_DATA_KEY_REGEX: Regex =
        Regex::new(r"^[a-zA-Z0-9._-]+$").expect("failed to init secret data key regex");
}

#[declarative_env(path = "./env_config.hjson")]
struct EnvConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct SecretSealRequest {
    name: String,
    namespace: String,
    certificate: String,
    data: HashMap<String, String>,
}

#[post("/secrets/seal")]
async fn seal(req_body: Json<SecretSealRequest>) -> impl Responder {
    if !RFC1123_REGEX.is_match(&req_body.name) {
        error!("400 Bad Request - Secret name is not RFC1123-compliant.");
        return HttpResponse::BadRequest().body("Secret name is not RFC1123 compliant.");
    }
    if !RFC1123_REGEX.is_match(&req_body.namespace) {
        error!("400 Bad Request - Secret namespace is not RFC1123-compliant.");
        return HttpResponse::BadRequest().body("Secret namespace is not RFC1123 complient.");
    }
    for key in req_body.data.keys() {
        if !SECRET_DATA_KEY_REGEX.is_match(key) {
            error!("400 Bad Request - Secret data key is not a valid key.");
            return HttpResponse::BadRequest().body("Secret data keys must be RFC1123 complient.");
        }
    }
    let secret_manifest =
        construct_secret_manifest(&req_body.name, &req_body.namespace, &req_body.data);
    let secret_manifest_path = format!("/tmp/{}-{}.yaml", &req_body.namespace, &req_body.name);
    let secret_manifest_file = match OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .read(true)
        .open(&secret_manifest_path)
    {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to open secret manifest file: {}", e);
            return HttpResponse::InternalServerError()
                .body(format!("failed to open secret manifest file: {}", e));
        },
    };
    if let Err(e) = std::fs::write(&secret_manifest_path, secret_manifest) {
        error!("Failed to write secret manifest: {}", e);
        return HttpResponse::InternalServerError()
            .body(format!("failed to write secret manifest: {}", e));
    }
    let tmp_cert_path = format!("/tmp/{}-{}-cert.pem", &req_body.namespace, &req_body.name);
    if let Err(e) = std::fs::write(&tmp_cert_path, &req_body.certificate) {
        error!("Failed to write certificate file: {}", e);
        return HttpResponse::InternalServerError()
            .body(format!("failed to write certificate file: {}", e));
    }
    let execution_result = match Command::new("kubeseal")
        .args(["--cert", &tmp_cert_path])
        .arg("--allow-empty-data")
        .stdin(secret_manifest_file)
        .output()
    {
        Ok(v) => v,
        Err(e) => {
            error!("failed to run kubeseal command: {}", e);
            return HttpResponse::InternalServerError()
                .body(format!("failed to run kubeseal command: {}", e));
        },
    };
    if execution_result.status.success() {
        info!("Secret sealed successfully");
        let result_stdout = std::str::from_utf8(&execution_result.stdout)
            .expect("The stdout of a shell should be UTF-8")
            .to_owned();
        HttpResponse::Ok().body(result_stdout)
    } else {
        let error_message = std::str::from_utf8(&execution_result.stderr)
            .unwrap_or("unknown error")
            .to_owned();
        error!("Failed to seal secret: {}", error_message);
        HttpResponse::InternalServerError().body(error_message)
    }
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().body("HEALTHY")
}

#[actix_web::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    let config = EnvConfig::from_env()?;
    env_logger::builder()
        .filter(None, LevelFilter::from_str(config.RUST_LOG())?)
        .init();
    let prometheus = PrometheusMetricsBuilder::new(&std::env!("CARGO_PKG_NAME").replace('-', "_"))
        .endpoint("/metrics")
        .build()
        .expect("the prometheus service failed to instantiate");
    let server = HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .wrap(prometheus.clone())
            .service(health)
            .service(seal)
    })
    .bind(("0.0.0.0", config.SERVER_PORT()))?
    .run();
    server.await.map_err(|it| it.into())
}

fn construct_secret_manifest(
    name: &str,
    namespace: &str,
    data: &HashMap<String, String>,
) -> String {
    let mut lines: Vec<String> = vec![
        "apiVersion: v1".into(),
        "kind: Secret".into(),
        "metadata:".into(),
        format!("  name: {}", name),
        format!("  namespace: {}", namespace),
        "type: Opaque".into(),
    ];
    if !data.is_empty() {
        lines.push("stringData:".into());
        let data_lines: Vec<String> = data
            .iter()
            .map(|entry| format!("  {}: |-\n    {}", entry.0, entry.1))
            .collect();
        lines.push(data_lines.join("\n"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod test {
    use crate::{RFC1123_REGEX, SECRET_DATA_KEY_REGEX};

    #[test]
    fn test_rfc1123_regex() {
        assert!(RFC1123_REGEX.is_match("abc"));
        assert!(RFC1123_REGEX.is_match("123"));
        assert!(RFC1123_REGEX.is_match("abc123"));
        assert!(!RFC1123_REGEX.is_match(""));
        assert!(!RFC1123_REGEX.is_match("/abc"));
        assert!(!RFC1123_REGEX.is_match("name_with_unserscores"));
        assert!(RFC1123_REGEX.is_match("abc-123"));
        assert!(!RFC1123_REGEX.is_match(
            "very-loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong-name"
        ));
        assert!(!RFC1123_REGEX.is_match("Name"));
        assert!(!RFC1123_REGEX.is_match("UPPERCASE"));
    }

    #[test]
    fn test_secret_data_key_regex() {
        assert!(SECRET_DATA_KEY_REGEX.is_match("testfile.txt"));
        assert!(SECRET_DATA_KEY_REGEX.is_match(".secret-file"));
        assert!(!SECRET_DATA_KEY_REGEX.is_match(""));
        assert!(SECRET_DATA_KEY_REGEX.is_match("UPPERCASE_NAME"));
        assert!(!SECRET_DATA_KEY_REGEX.is_match("file/with/path.txt"));
    }
}
