// 以下代码复制自：
// https://github.com/shuttle-hq/shuttle/blob/33b9e55a/services/shuttle-salvo/src/lib.rs
// 原作品版权由其原作者保留，按 Apache License 2.0 授权。
// 完整许可证文本见项目根目录的 LICENSE-APACHE 文件。

use salvo::Listener;
use shuttle_runtime::Error;
use std::net::SocketAddr;

pub use salvo;

/// A wrapper type for [salvo::Router] so we can implement [shuttle_runtime::Service] for it.
pub struct SalvoService(pub salvo::Router);

#[shuttle_runtime::async_trait]
impl shuttle_runtime::Service for SalvoService {
    /// Takes the router that is returned by the user in their [shuttle_runtime::main] function
    /// and binds to an address passed in by shuttle.
    async fn bind(mut self, addr: SocketAddr) -> Result<(), Error> {
        let listener = salvo::conn::TcpListener::new(addr).bind().await;

        salvo::Server::new(listener).serve(self.0).await;

        Ok(())
    }
}

impl From<salvo::Router> for SalvoService {
    fn from(router: salvo::Router) -> Self {
        Self(router)
    }
}

pub type ShuttleSalvo = Result<SalvoService, Error>;
