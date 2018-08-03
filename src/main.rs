#[macro_use]
extern crate lazy_static;
extern crate handlebars;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate reqwest;
extern crate regex;
extern crate chrono;
extern crate actix_web;
use std::env;

mod api;

use actix_web::{http, server, App, Path, Responder, fs};
use handlebars::Handlebars;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;


fn fill_template(str_stop_id: &str) -> String {
    let file = File::open("static/index.html").expect("oe");
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents).unwrap();
    let reg = Handlebars::new();

    let result: String = api::run(str_stop_id);

    if result.contains("  -  ") {
        let list: Vec<&str> = result.split("  -  ").collect();
        let (stopname, prediction): (&str, &str) = (list[0], list[1]);

        reg.render_template(contents.as_str(), &json!({
        "StopId": str_stop_id,
        "StopName": stopname,
        "Prediction": prediction
        }))
        .unwrap()
    }
    else {
        let stopname = result;
        let prediction = "";

        reg.render_template(contents.as_str(), &json!({
        "StopId": str_stop_id,
        "StopName": stopname,
        "Prediction": prediction
        }))
        .unwrap()
    }
}


fn index(info: Path<(u32,)>) -> impl Responder {
    let str_stop_id: &str = &info.0.to_string();

    actix_web::HttpResponse::Ok()
       .content_type("text/html")
       .body(fill_template(str_stop_id))

}

fn arduino_get(info: Path<(u32,)>) -> impl Responder {
    let str_stop_id: &str = &info.0.to_string();
    api::run(str_stop_id)
}

fn main() {
    server::new(
        || App::new()
            .handler(
            "/static",
            fs::StaticFiles::new("static")
                .unwrap()
                .show_files_listing())
            .route("/{id}", http::Method::GET, index)
            .route("/cta/{id}", http::Method::GET, arduino_get)
    )
        .bind("0.0.0.0:8080").unwrap()
        .run();
}