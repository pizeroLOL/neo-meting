use std::{
    ops::Deref,
    sync::{Arc, LazyLock},
};

use neo_metting::{netease::Netease, MetingApi, MetingSearchOptions};
use salvo::{
    async_trait,
    conn::TcpListener,
    http::StatusError,
    writing::{Json, Redirect},
    Depot, FlowCtrl, Handler, Listener, Request, Response, Router, Server,
};
use tokio::sync::{RwLock, Semaphore};
use tracing::warn;

pub trait Then {
    fn then<O>(self, f: impl FnOnce(Self) -> O) -> O
    where
        Self: std::marker::Sized,
    {
        f(self)
    }

    fn change_self(mut self, f: impl FnOnce(&mut Self)) -> Self
    where
        Self: std::marker::Sized,
    {
        f(&mut self);
        self
    }
}
impl<T> Then for T {}

fn prosess_meting_error(file: &str, line: u32, e: neo_metting::Error) -> StatusError {
    use neo_metting::Error as E;
    warn!("{file}:{line}: {e:?}");
    match e {
        E::Remote(_) => StatusError::bad_gateway(),
        E::Server(_) => StatusError::internal_server_error(),
        E::Encode { engine: _, msg: _ } => StatusError::internal_server_error(),
        E::NoField(_) => StatusError::bad_gateway(),
        E::TypeMismatch {
            feild: _,
            target: _,
        } => StatusError::bad_gateway(),
        E::None => StatusError::not_found(),
        E::Unimplemented => StatusError::not_found(),
    }
}

macro_rules! handle_error {
    ($e:expr) => {
        prosess_meting_error(file!(), line!(), $e)
    };
}

static RETRY: LazyLock<Arc<RwLock<u8>>> = LazyLock::new(|| Arc::new(RwLock::new(0)));

trait SalvoMeting: MetingApi
where
    Self: Send + Sync + 'static,
{
    fn get_pic(self: Arc<Self>) -> impl Handler {
        struct Handle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Handle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting> Handler for Handle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let url = self.pic(param).await;
                match url {
                    Ok(o) => res.render(Redirect::found(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Handle(self.clone())
    }
    fn get_lrc(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let url = self.lrc(param).await;
                match url {
                    Ok(o) => res.render(o),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }
    fn get_url(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let url = self.url(param).await;
                match url {
                    Ok(o) => res.render(Redirect::found(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }

    fn get_song(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let server = req.uri();
                let scheme = server
                    .scheme_str()
                    .map(|sheme| format!("{sheme}://"))
                    .unwrap_or(format!("http"));
                let Some(auth) = server.authority().map(|auth| auth.as_str()) else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let client = S::name();
                let url = self
                    .song(
                        param,
                        |pid| format!("{scheme}{auth}/{client}/pic/{pid}",),
                        |lid| format!("{scheme}{auth}/{client}/lrc/{lid}",),
                        |uid| format!("{scheme}{auth}/{client}/url/{uid}",),
                    )
                    .await;
                match url {
                    Ok(o) => res.render(Json(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }

    fn get_playlist(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let server = req.uri();
                let scheme = server
                    .scheme_str()
                    .map(|sheme| format!("{sheme}://"))
                    .unwrap_or(format!("http"));
                let Some(auth) = server.authority().map(|auth| auth.as_str()) else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let client = S::name();
                let url = self
                    .playlist(
                        param,
                        *RETRY.read().await,
                        |pid| format!("{scheme}{auth}/{client}/pic/{pid}",),
                        |lid| format!("{scheme}{auth}/{client}/lrc/{lid}",),
                        |uid| format!("{scheme}{auth}/{client}/url/{uid}",),
                    )
                    .await;
                match url {
                    Ok(o) => res.render(Json(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }
    #[allow(unused)]
    fn get_artist(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let server = req.uri();
                let scheme = server
                    .scheme_str()
                    .map(|sheme| format!("{sheme}://"))
                    .unwrap_or(format!("http"));
                let Some(auth) = server.authority().map(|auth| auth.as_str()) else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let client = S::name();
                let url = self
                    .artist(
                        param,
                        |pid| format!("{scheme}{auth}/{client}/pic/{pid}",),
                        |lid| format!("{scheme}{auth}/{client}/lrc/{lid}",),
                        |uid| format!("{scheme}{auth}/{client}/url/{uid}",),
                    )
                    .await;
                match url {
                    Ok(o) => res.render(Json(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }
    fn get_search(self: Arc<Self>) -> impl Handler {
        struct Hendle<S: SalvoMeting>(Arc<S>);
        impl<S: SalvoMeting> Deref for Hendle<S> {
            type Target = Arc<S>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        #[async_trait]
        impl<S: SalvoMeting + Sync + Send + 'static> Handler for Hendle<S> {
            async fn handle(
                &self,
                req: &mut Request,
                _depot: &mut Depot,
                res: &mut Response,
                _ctrl: &mut FlowCtrl,
            ) {
                let Some(param) = req.param::<&str>("id") else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let server = req.uri();
                let scheme = server
                    .scheme_str()
                    .map(|sheme| format!("{sheme}://"))
                    .unwrap_or(format!("http"));
                let Some(auth) = server.authority().map(|auth| auth.as_str()) else {
                    res.render(StatusError::bad_request());
                    return;
                };
                let client = S::name();
                let options = MetingSearchOptions {
                    limit: 30,
                    page: 1,
                    r#type: 0,
                };
                let url = self
                    .search(
                        param,
                        options,
                        |pid| format!("{scheme}{auth}/{client}/pic/{pid}",),
                        |lid| format!("{scheme}{auth}/{client}/lrc/{lid}",),
                        |uid| format!("{scheme}{auth}/{client}/url/{uid}",),
                    )
                    .await;
                match url {
                    Ok(o) => res.render(Json(o)),
                    Err(e) => res.render(handle_error!(e)),
                }
            }
        }
        Hendle(self.clone())
    }
}

impl<T: MetingApi> SalvoMeting for T {}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let semaphore = Semaphore::const_new(8);
    let netease = semaphore.then(Arc::new).then(Netease::new).then(Arc::new);
    let netease_route = Router::with_path(Netease::name())
        .push(Router::with_path("search/<id>").get(netease.clone().get_search()))
        .push(Router::with_path("playlist/<id>").get(netease.clone().get_playlist()))
        .push(Router::with_path("song/<id>").get(netease.clone().get_song()))
        // .push(Router::with_path("artist/<id>").get(netease.clone().get_artist()))
        .push(Router::with_path("lrc/<id>").get(netease.clone().get_lrc()))
        .push(Router::with_path("pic/<id>").get(netease.clone().get_pic()))
        .push(Router::with_path("url/<id>").get(netease.clone().get_url()));
    let acceptor = TcpListener::new("127.0.0.1:5811").bind().await;
    Server::new(acceptor).serve(netease_route).await;
}
