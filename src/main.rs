use std::{collections::HashSet, error, fs, path::Path, str::FromStr};

use hyper::{body::HttpBody, Body, Client, Request, Response};
use hyper_tls::HttpsConnector;
use log::{Level, Metadata, Record, info, log};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use warp::{
    hyper::{body::Bytes, HeaderMap, Method},
    Filter,
};

use crate::{
    pep_427::WheelInfo,
    pep_440::{SpecifierSet, Version},
};

mod pep_427;
mod pep_440;
mod pep_503;

// TODO: figure out pattern to differentiate between
// actionable errors (e.g. failed to parse version)
// vs. unactionable errors (e.g. file doesn't exist)

#[derive(Serialize, Deserialize, Debug)]
struct PackageConfig {
    release_denylist: Vec<String>,
    version_limits: String,
}

impl PackageConfig {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn error::Error>> {
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }
}

async fn forward_upstream<S: AsRef<str>>(
    uri: S,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response<String> {
    // TODO: Make it so you can parse partial input here
    if method != "GET" {
        return Response::builder()
            .status(400)
            .body("can only forward GET requests for now".to_owned())
            .unwrap();
    }

    let mut request = Request::builder().method(Method::GET).uri(uri.as_ref());
    for (header, value) in headers.into_iter() {
        let header = if let Some(header) = header {
            header
        } else {
            continue;
        };

        if header == "host" || header == "accept-encoding" {
            // host -> makes cURL commands fail
            // accept-encoding -> makes us get binary data back
            continue;
        }

        request = request.header(header, value);
    }
    let request = request.body(Body::from(body)).unwrap();

    // TODO: make the request of this request flow prettier
    let https = HttpsConnector::new();
    let client = Client::builder().build(https);
    let mut res = client
        .request(request)
        .await
        .expect("failed to make HTTP request");

    let mut response = Vec::<u8>::new();
    while let Some(Ok(chunk)) = res.body_mut().data().await {
        response.extend(chunk);
    }
    let response_str = String::from_utf8(response).unwrap();

    let mut our_res = Response::builder().status(res.status());
    for (header, value) in res.headers() {
        our_res = our_res.header(header, value);
    }
    our_res.body(response_str).unwrap()
}

async fn handle_root_index(method: Method, headers: HeaderMap, body: Bytes) -> Response<String> {
    info!("{} /simple/", method);

    // TODO: this is REALLY slow right now. optimize!
    let mut res = forward_upstream("https://pypi.org/simple/", method, headers, body).await;
    let root_index = pep_503::RootIndex::from_str(res.body()).unwrap();

    let body = root_index.to_string();
    res.headers_mut().remove("content-length");
    (*res.body_mut()) = body;

    res
}

async fn handle_package_index(
    package: String,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response<String> {
    info!("{} /simple/{}/", method, package);

    let uri = format!("https://pypi.org/simple/{package}/");
    let mut res = forward_upstream(&uri, method, headers, body).await;
    let mut package_index = pep_503::PackageIndex::from_str(res.body()).unwrap();

    // TODO: forwarding request + loading JSON can happen in parallel
    if let Ok(package_config) = PackageConfig::load(format!("fixtures/{package}.json")) {
        let denylisted_releases = package_config
            .release_denylist
            .into_iter()
            .collect::<HashSet<String>>();

        let specifier_set = SpecifierSet::from_str(&package_config.version_limits).unwrap();

        // TODO: filter this in place to not copy memory around
        let mut releases = vec![];
        for release in package_index.releases.into_iter() {
            if denylisted_releases.contains(&release.name) {
                // TODO: this should include wildcards,
                continue;
            }

            if let Ok(wheel_info) = WheelInfo::from_str(&release.name) {
                let version = Version::from_str(&wheel_info.version).unwrap();
                if !specifier_set.contains(&version) {
                    continue;
                }
            }

	    let sdist_pkg = if release.name.ends_with(".tar.gz") {
		Some(&release.name[..release.name.len() - ".tar.gz".len()])
	    } else if release.name.ends_with(".zip") {
		Some(&release.name[..release.name.len() - ".zip".len()])
	    } else if release.name.ends_with(".sdist") {
		Some(&release.name[..release.name.len() - ".sdist".len()])
	    } else {
		None
	    };
	    if let Some(sdist_pkg) = sdist_pkg {
		let (_, version_str) = sdist_pkg.split_once('-').unwrap();
		match Version::from_str(version_str) {
		    Err(e) => {
			log!(Level::Warn, "failed to parse version str for `{}`: {}", sdist_pkg, e);
			continue;
		    },
		    Ok(version) => {
			if !specifier_set.contains(&version) {
			    continue;
			}
		    },
		}
	    }

	    if release.name.ends_with(".egg") {
		// Opinionated choice: we don't care about eggs anymore!
		// We have a standardized built distribution format in wheels.
		// If a project only publishes eggs you probably don't want to use it.
		continue;
	    }

            releases.push(release);
        }
        package_index.releases = releases;

        let body = package_index.to_string();
        res.headers_mut().remove("content-length");
        (*res.body_mut()) = body;
    }

    // TODO: unconditionally replace the body with the package_index result?
    res
}

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

#[tokio::main]
async fn main() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Info))
        .unwrap();

    let capture_request = warp::filters::method::method()
        .and(warp::header::headers_cloned())
        .and(warp::filters::body::bytes());

    let root_index = warp::path!("simple")
        .and(capture_request)
        .and(warp::get())
        .then(handle_root_index);

    let package_index = warp::path!("simple" / String)
        .and(warp::get())
        .and(capture_request)
        .then(handle_package_index);

    let router = root_index.or(package_index);
    println!("Serving 127.0.0.1:8080...");
    warp::serve(router).run(([127, 0, 0, 1], 8080)).await;
}
