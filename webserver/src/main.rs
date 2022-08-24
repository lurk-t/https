use http::{
    header::{CONTENT_DISPOSITION, CONTENT_TYPE},
    HeaderValue, Request, Response, StatusCode,
};
use hyper::{server::conn::Http, service::service_fn, Body};
use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod, SslOptions, SslVersion,SslSessionCacheMode};
use std::{
    collections::HashMap, convert::Infallible, error::Error, fs, fs::File, io, io::Read, pin::Pin,
    sync::Arc,
};
use tokio::net::TcpListener;
use tokio_openssl::SslStream;
use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(version = "1.0", author = "anon-data@anon.com")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "ec_key.pem")]
    key: String,
    #[clap(short, long, default_value = "cert.pem")]
    cert: String,
    #[clap(short, long, default_value = "0.0.0.0:8080")]
    address: String,
}

async fn service(
    req: Request<Body>,
    files: HashMap<String, Vec<u8>>,
) -> Result<Response<Body>, Infallible> {
    if let Some(file) = files.get_key_value(&req.uri().path()[1..]) {
        let mut response = Response::new(Body::from(file.1.to_owned()));
        response.headers_mut().insert(
            CONTENT_DISPOSITION,
            match HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file.0.to_owned()))
            {
                Ok(value) => value,
                Err(_) => HeaderValue::from_static("attachment; filename=\"Download\""),
            },
        );
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        *response.status_mut() = StatusCode::OK;
        return Ok(response);
    } else if req.uri().path() == "/" {
        let mut body = String::new();
        for name in files.keys() {
            body.push_str(&format!("<a href=\"{0}\">Download {0}</a><br>", name));
        }
        let mut response = Response::new(Body::from(body));
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
        *response.status_mut() = StatusCode::OK;
        Ok(response)
    } else {
        let mut response = Response::new(Body::from("Not Found!"));
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
        *response.status_mut() = StatusCode::NOT_FOUND;
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();
    let file_names = fs::read_dir("files")?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;
    let mut files = HashMap::new();
    for file_name in file_names {
        let mut f = File::open(&file_name.clone())?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        files.insert(
            file_name.file_name().unwrap().to_str().unwrap().to_string(),
            data,
        );
    }
    let mut acceptor = SslAcceptor::mozilla_modern(SslMethod::tls())?;
    acceptor.set_private_key_file(opts.key, SslFiletype::PEM)?;
    acceptor.set_certificate_chain_file(opts.cert)?;
    acceptor.check_private_key()?;
    acceptor.clear_options(SslOptions::NO_TLSV1_3);
    acceptor.set_session_cache_mode(SslSessionCacheMode::OFF);
    //acceptor.set_options(SslOptions::NO_TICKET);
    acceptor.set_min_proto_version(Some(SslVersion::TLS1_3))?;
    let acceptor = Arc::new(acceptor.build());
    let listener = TcpListener::bind(opts.address).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let ssl = Ssl::new(acceptor.context())?;
        let mut stream = SslStream::new(ssl, socket)?;
        if let Err(e) = Pin::new(&mut stream).accept().await {
            eprintln!("{}: Do you use TLS 1.3?", e);
            continue;
        }
        let files = files.clone();
        tokio::spawn(async move {
            if let Err(http_err) = Http::new()
                .http1_only(true)
                .http1_keep_alive(true)
                .serve_connection(stream, service_fn(|s| service(s, files.clone())))
                .await
            {
                eprintln!("Error while serving HTTP connection: {}", http_err);
            }
        });
    }
    // Ok(())
}
