use axum::{
    extract::{rejection::JsonRejection, FromRequest, MatchedPath, Request, State, Path, Query}, 
    http::{version, StatusCode}, 
    response::{Html, IntoResponse, Response}, 
    routing::{get, post}, Json, Router, body::Body
};
use std::io::{BufWriter, Cursor};
use std::sync::{Arc, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wz_reader::{WzNodeArc, WzNodeCast, node, util::resolve_base, version::WzMapleVersion, property};
use image::ImageFormat;

#[derive(Clone)]
pub struct ServerState {
    pub wz_root: Arc<RwLock<Option<WzNodeArc>>>,
}

#[tokio::main]
async fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let default_port = "3000".to_string();
    let port = args.get(1).unwrap_or(&default_port);

    let state = ServerState {
        wz_root: Arc::new(RwLock::new(None)),
    };

    let app = Router::new()
        .route("/", get(handler))
        .route("/init_wz_root", post(init_wz_root))
        .route("/get_json/*path", get(get_json))
        .route("/get_image/*path", get(get_image))
        .route("/get_sound/*path", get(get_sound))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hi</h1>")
}

/* init wz part */
enum InitWzError {
    MissingParam,
    ParseError,
    VersionError,
    IoError,
}
impl IntoResponse for InitWzError {
    fn into_response(self) -> Response {
        match self {
            InitWzError::MissingParam => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("should passing path and version".into()).unwrap()
            }
            InitWzError::ParseError => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("wz parse error".into()).unwrap()
            }
            InitWzError::VersionError => {
                Response::builder().status(StatusCode::BAD_REQUEST).body("passing wrong wz version".into()).unwrap()
            }
            InitWzError::IoError => {
                Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body("file error".into()).unwrap()
            }
        }
    }
}
async fn init_wz_root(
    State(ServerState { wz_root }): State<ServerState>,
    Json(body): Json<Value>,
) -> Response {
    let base_path = body.get("path").and_then(|v| v.as_str());
    let version = body.get("version").and_then(|v| v.as_str());

    if base_path.is_none() || version.is_none() {
        return InitWzError::MissingParam.into_response();
    }

    let version = match version.unwrap() {
        "BMS" => Some(WzMapleVersion::BMS),
        "GMS" => Some(WzMapleVersion::GMS),
        "EMS" => Some(WzMapleVersion::EMS),
        _ => None,
    };

    let base_path = base_path.unwrap();

    if base_path.is_empty() {
        return InitWzError::MissingParam.into_response();
    }

    let result = resolve_base(base_path, version);
    if result.is_err() {
        return InitWzError::IoError.into_response();
    }
    let base_node = result.unwrap();
    let mut wz_root = wz_root.write().unwrap();
    *wz_root = Some(base_node);

    return StatusCode::OK.into_response();
}

/* grabe json part */
#[derive(Deserialize)]
struct GetJsonParam {
    simple: Option<bool>,
    force_parse: Option<bool>,
}

async fn get_json(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get path's json: {}", path);
    let is_simple = param.simple.unwrap_or(false);
    let force_parse = param.force_parse.unwrap_or(false);

    let wz_root = wz_root.read().unwrap();

    if wz_root.is_none() {
        return Response::builder().status(StatusCode::BAD_REQUEST).body::<Body>("wz uninitialized".into()).unwrap().into_response()
    }

    let wz_root = wz_root.as_ref().unwrap();
    let wz_root = wz_root.read().unwrap();

    let target = if force_parse {
        wz_root.at_path_parsed(&path)
    } else {
        wz_root.at_path(&path).ok_or(node::Error::NodeNotFound)
    };

    if target.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = target.unwrap();

    if force_parse {
        node::parse_node(&target).unwrap()
    }

    let json = if is_simple {
        target.read().unwrap().to_simple_json()
    } else {
        target.read().unwrap().to_json()
    };

    let json = json.unwrap();

    return Response::builder().status(StatusCode::OK).header("content-type", "application/json;charset=utf-8").body::<Body>(json.to_string().into()).unwrap().into_response();
}

/* grabe image part */
async fn get_image(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get image: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let wz_root = wz_root.read().unwrap();

    if wz_root.is_none() {
        return Response::builder().status(StatusCode::BAD_REQUEST).body::<Body>("wz uninitialized".into()).unwrap().into_response()
    }

    let wz_root = wz_root.as_ref().unwrap();
    let wz_root = wz_root.read().unwrap();

    let target = if force_parse {
        wz_root.at_path_parsed(&path)
    } else {
        wz_root.at_path(&path).ok_or(node::Error::NodeNotFound)
    };

    if target.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = target.unwrap();

    if force_parse {
        node::parse_node(&target).unwrap()
    }

    let target_read = target.read().unwrap();
    
    if let Some(_) = target_read.try_as_png() {
        let img = property::get_image(&target);

        if img.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        let img = img.unwrap();

        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        img.write_to(&mut buf, ImageFormat::Bmp).expect("write image error");

        return Response::builder().status(StatusCode::OK).header("content-type", "image/bmp").body::<Body>(buf.into_inner().unwrap().into_inner().into()).unwrap().into_response();
    } 

    return StatusCode::BAD_REQUEST.into_response();
}


/* grabe sound part */
async fn get_sound(
    State(ServerState { wz_root }): State<ServerState>,
    Path(path): Path<String>,
    Query(param): Query<GetJsonParam>,
) -> Response {
    println!("try to get sound: {}", path);
    let force_parse = param.force_parse.unwrap_or(false);

    let wz_root = wz_root.read().unwrap();

    if wz_root.is_none() {
        return Response::builder().status(StatusCode::BAD_REQUEST).body::<Body>("wz uninitialized".into()).unwrap().into_response()
    }

    let wz_root = wz_root.as_ref().unwrap();
    let wz_root = wz_root.read().unwrap();

    let target = if force_parse {
        wz_root.at_path_parsed(&path)
    } else {
        wz_root.at_path(&path).ok_or(node::Error::NodeNotFound)
    };

    if target.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = target.unwrap();

    if force_parse {
        node::parse_node(&target).unwrap()
    }

    let target_read = target.read().unwrap();
    
    if let Some(sound) = target_read.try_as_sound() {
        let sound_buf = sound.get_buffer();

        let mini = match sound.sound_type {
            property::WzSoundType::Wav => "audio/wav",
            _ => "audio/mpeg",
        };

        return Response::builder().status(StatusCode::OK).header("content-type", mini).body::<Body>(sound_buf.into()).unwrap().into_response();
    } 

    return StatusCode::BAD_REQUEST.into_response();
}