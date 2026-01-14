use std::future::Future;

pub mod netease;

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

#[derive(Debug)]
pub enum Error {
    Remote(String),
    Server(String),
    Encode {
        engine: &'static str,
        msg: String,
    },
    NoField(&'static str),
    TypeMismatch {
        feild: &'static str,
        target: &'static str,
    },
    None,
    Unimplemented,
}

pub async fn retry<I, O, E, Task, GenTaskFunc, OnErrFunc>(
    limit: u8,
    input: I,
    task: GenTaskFunc,
    on_error: OnErrFunc,
) -> Result<O, E>
where
    I: Clone,
    Task: Future<Output = Result<O, E>>,
    GenTaskFunc: Fn(I) -> Task,
    OnErrFunc: Fn(E),
{
    let mut counter = 0;
    loop {
        let result = task(input.clone()).await;
        match result {
            Ok(o) => break Ok(o),
            Err(e) if counter < limit => {
                on_error(e);
                counter += 1
            }
            Err(e) => break Err(e),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MetingSearchOptions {
    pub limit: usize,
    pub page: usize,
    pub r#type: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MetingSong {
    name: String,
    artist: String,
    url: String,
    pic: String,
    lrc: String,
}

pub trait MetingApi
where
    Self: Sized + Clone + Sync + Send + 'static,
{
    fn name() -> &'static str;
    fn url(&self, _id: &str) -> impl Future<Output = Result<String, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
    fn pic(&self, _id: &str) -> impl Future<Output = Result<String, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
    fn lrc(&self, _id: &str) -> impl Future<Output = Result<String, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
    fn song(
        &self,
        _id: &str,
        _pic: impl Fn(&str) -> String + Sync + Send,
        _lrc: impl Fn(&str) -> String + Sync + Send,
        _url: impl Fn(&str) -> String + Sync + Send,
    ) -> impl Future<Output = Result<MetingSong, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }

    fn artist(
        &self,
        _id: &str,
        _pic: impl Fn(&str) -> String + Send + Sync,
        _lrc: impl Fn(&str) -> String + Send + Sync,
        _url: impl Fn(&str) -> String + Send + Sync,
    ) -> impl Future<Output = Result<Vec<MetingSong>, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
    fn playlist(
        &self,
        _id: &str,
        _retry: u8,
        _pic: impl Fn(&str) -> String + Send + Sync,
        _lrc: impl Fn(&str) -> String + Send + Sync,
        _url: impl Fn(&str) -> String + Send + Sync,
    ) -> impl Future<Output = Result<Vec<MetingSong>, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
    fn search(
        &self,
        _keyword: &str,
        _option: MetingSearchOptions,
        _pic: impl Fn(&str) -> String + Send,
        _lrc: impl Fn(&str) -> String + Send,
        _url: impl Fn(&str) -> String + Send,
    ) -> impl Future<Output = Result<Vec<MetingSong>, Error>> + Send {
        async { Err(Error::Unimplemented) }
    }
}
