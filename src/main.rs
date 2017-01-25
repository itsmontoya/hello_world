extern crate env_logger;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate futures_cpupool;

use std::io;
use std::sync;
use std::io::prelude::*;
use std::fs::File;
use std::result;

use hyper::header::{ContentLength, ContentType};
use hyper::server::{Http, Service, Request, Response};
use hyper::mime::{Mime, TopLevel, SubLevel};

use futures::future;
use futures::Stream;
use futures::Future;
use futures_cpupool::CpuPool;

#[macro_use]
extern crate serde_derive;

static ERR_INVALID_GREETING: &'static str = "invalid greeting provided";
static ERR_INVALID_NAME: &'static str = "invalid name provided";
static ERR_INVALID_METHOD: &'static str = "invalid method used";

fn main() {
    drop(env_logger::init());

    let mux = sync::Arc::new(sync::RwLock::new(APIData {
        svc: LBService {
            name: String::from(""),
            greeting: String::from(""),
        },
    }));

    let pool = CpuPool::new(4);

    {
        let mc = mux.clone();
        let mut d = mc.write().unwrap();
        d.svc.name = String::from("Josh");
        d.svc.greeting = String::from("Hai!");
    }

    let addr = "127.0.0.1:8080".parse().unwrap();
    let server = Http::new()
        .bind(&addr, move || {
            let mc = mux.clone();
            let pc = pool.clone();

            Ok(Responder { d: mc, p: pc })
        })
        .unwrap();

    println!("Listening on http://{}", server.local_addr().unwrap());
    server.run().unwrap();
}

fn handle_dashboard() -> Result {
    let mut file = match File::open("./tmpls/dashboard.mustache.html") {
        Ok(file) => file,
        Err(err) => return Err(err.to_string()),
    };

    let mut s = String::new();
    match file.read_to_string(&mut s) {
        Ok(_) => {}
        Err(err) => return Err(err.to_string()),
    }

    Ok(Response::new()
        .with_header(ContentLength(s.len() as u64))
        .with_header(ContentType::html())
        .with_body(s.into_bytes()))
}



fn handle_404() -> Result {
    let s = String::from("404, not found.");

    Ok(Response::new()
        .with_status(hyper::StatusCode::NotFound)
        .with_header(ContentLength(s.len() as u64))
        .with_header(ContentType::html())
        .with_body(s.into_bytes()))
}

fn get_error_resp(code: hyper::StatusCode, msg: &str) -> Response {
    Response::new()
        .with_status(code)
        .with_header(ContentLength(msg.len() as u64))
        .with_body(msg.to_string().into_bytes())
}


struct Responder {
    d: Data,
    p: CpuPool,
}

impl Service for Responder {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = ::futures::Finished<Response, hyper::Error>;

    fn call(&self, _request: Request) -> Self::Future {
        let res = match _request.path() {
            "/" => handle_dashboard(),
            "/api/service" => self.handle_service(_request),
            _ => handle_404(),
        };

        match res {
            Ok(resp) => future::ok(resp),
            Err(err) => future::ok(get_error_resp(hyper::StatusCode::BadRequest, &err)),
        }
    }
}

impl Responder {
    fn handle_service(&self, req: Request) -> Result {
        match req.method() {
            &hyper::Get => return self.get_service(req),  
            &hyper::Put => return self.put_service(req),
            _ => return Err(ERR_INVALID_METHOD.to_string()),
        }
    }

    fn get_service(&self, req: Request) -> Result {
        let d = match self.d.read() {
            Ok(d) => d,
            Err(err) => return Err(err.to_string()),
        };

        let s = match d.svc.to_json() {
            Ok(s) => s,
            Err(err) => return Err(err),
        };

        Ok(Response::new()
            .with_status(hyper::StatusCode::Ok)
            .with_header(ContentLength(s.len() as u64))
            .with_header(ContentType::json())
            .with_body(s.into_bytes()))
    }

    fn put_service(&self, req: Request) -> Result {
        let body = req.body();
        let body_vec: Vec<u8> = Vec::new();
        println!("{}", "About to call some shit");
        let v = body.fold(body_vec, |mut acc, chunk| {
                acc.extend_from_slice(chunk.as_ref());
                println!("Extending");
                Ok::<Vec<u8>, hyper::Error>(acc)
            });
            /*
            .and_then(move |body_vec| {
                // I know I'm using tons of unwrap here, I'll get proper error handling soon
                let body_str = String::from_utf8(body_vec).unwrap();
                println!("{}", body_str);
                let nv: LBService = serde_json::from_str(&body_str).unwrap();
                Ok(nv)
            })
            .and_then(|nv| {
                let mut d = self.d.write().unwrap();
                d.svc.greeting = nv.greeting;
                d.svc.name = nv.name;
                let res_str = String::from("{ \"success\" : true }");
                Ok(Response::new()
                    .with_status(hyper::StatusCode::Ok)
                    .with_header(ContentLength(res_str.len() as u64))
                    .with_header(ContentType::json())
                    .with_body(res_str.into_bytes()))
            });
            */
        //            .or_else(|e| {
        //              // maybe handle different errors however you'd like
        //            Ok(Response::new()
        //          .with_status(hyper::StatusCode::BadRequest)
        //        // or maybe StatusCode::InternalServerError
        //      .with_body("something bad happened"))
        // });
        v.wait();
        Err(String::from("Merrp"))
    }
}

type Result = result::Result<Response, String>;
type JSONResult = result::Result<String, String>;
type BoolResult = result::Result<bool, String>;

type Data = sync::Arc<sync::RwLock<APIData>>;

struct APIData {
    svc: LBService,
}

#[derive(Serialize, Deserialize, Debug)]
struct LBService {
    greeting: String,
    name: String,
}

impl LBService {
    fn is_valid(&self) -> BoolResult {
        if self.greeting.len() == 0 {
            return Err(ERR_INVALID_GREETING.to_string());
        }

        if self.name.len() == 0 {
            return Err(ERR_INVALID_NAME.to_string());
        }

        Ok(true)
    }

    fn to_json(&self) -> JSONResult {
        let s = match serde_json::to_string(self) {
            Ok(s) => s,
            Err(err) => return Err(err.to_string()),
        };

        Ok(s)
    }
}