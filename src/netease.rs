use std::{
    collections::HashMap,
    fmt::{Display, Write},
    string::FromUtf8Error,
    sync::Arc,
};

use base64::{prelude::BASE64_STANDARD, Engine};
use openssl::{
    error::ErrorStack,
    rsa::{Padding, Rsa},
    symm::{encrypt, Cipher},
};
use rand::{rand_core::OsError, rngs::OsRng, TryRngCore};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, ClientBuilder,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{AcquireError, Semaphore};

#[cfg(feature = "random-ip")]
use rand::Rng;

use crate::{Error, MetingApi, MetingSearchOptions, MetingSong, Then};

#[derive(Debug)]
pub enum ParseErr {
    ImportPubKey(ErrorStack),
    EncodeSource(ErrorStack),
    EncodeRevStr(FromUtf8Error),
    EncodeData(ErrorStack),
    EncodeKey(ErrorStack),
    GenRandomNumber(OsError),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WeapiEncoder {
    params: String,
    enc_sec_key: String,
}

impl WeapiEncoder {
    pub fn try_from_str(input: &str) -> Result<Self, ParseErr> {
        let iv = b"0102030405060708";
        // let mut body = Vec::new();
        let cbc = Cipher::aes_128_cbc();
        let mut skey = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut skey)
            .map_err(ParseErr::GenRandomNumber)?;
        let base62 = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
            .as_bytes()
            .to_vec();
        let skey = skey
            .into_iter()
            .map(|index| base62[(index % 62) as usize])
            .collect::<Vec<_>>();

        let params = input
            .as_bytes()
            .then(|source| encrypt(cbc, b"0CoJUm6Qyw8W8jud", Some(iv), source))
            .map_err(ParseErr::EncodeSource)?
            .then(|data| BASE64_STANDARD.encode(data))
            .bytes()
            .collect::<Vec<_>>()
            .then(|data| encrypt(cbc, &skey, Some(iv), &data))
            .map_err(ParseErr::EncodeData)?
            .then(|output| BASE64_STANDARD.encode(output));

        let skey = skey
            .change_self(|skey| skey.reverse())
            .then(String::from_utf8)
            .map_err(ParseErr::EncodeRevStr)?;
        let rsa = Rsa::public_key_from_pem(include_bytes!("cert/netease.pub"))
            .map_err(ParseErr::ImportPubKey)?;
        let mut enc_sec_key = vec![0; rsa.size() as usize];
        [vec![0u8; 128 - skey.len()], skey.as_bytes().to_vec()]
            .concat()
            .then(|y| {
                rsa.public_encrypt(&y, &mut enc_sec_key, Padding::NONE)
                    .map_err(ParseErr::EncodeKey)
            })?;
        let enc_sec_key = hex::encode(enc_sec_key);
        Ok(Self {
            params,
            enc_sec_key,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Playlist<'a> {
    id: &'a str,
    offset: &'a str,
    total: &'a str,
    limit: &'a str,
    n: &'a str,
}

impl<'a> Playlist<'a> {
    pub(crate) fn new(id: &'a str) -> Self {
        Self {
            id,
            offset: "0",
            total: "True",
            limit: "9999",
            n: "9999",
        }
    }
}

impl Display for Playlist<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SongReq {
    c: String,
}

impl SongReq {
    pub(crate) fn new(c: String) -> Self {
        Self { c }
    }
}

impl Display for SongReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SongItem {
    pub id: u64,
    pub v: u8,
}

impl SongItem {
    pub(crate) fn new(id: u64) -> Self {
        Self { id, v: 0 }
    }
}

impl Display for SongItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SongFileReq {
    ids: Vec<String>,
    /// 记得 * 1000，不然会导致没有数据然后 502
    br: u64,
}

impl Display for SongFileReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Serialize)]
struct LrcReq<'a> {
    id: &'a str,
    os: &'a str,
    lv: isize,
    kv: isize,
    tv: isize,
    rv: isize,
    yv: usize,

    #[serde(rename = "camleCase")]
    show_role: &'a str,
    cp: &'a str,
    e_r: &'a str,
}

impl<'a> LrcReq<'a> {
    pub(crate) fn new(id: &'a str) -> Self {
        Self {
            id,
            os: "pc",
            lv: -1,
            kv: -1,
            tv: -1,
            rv: -1,
            yv: 1,
            show_role: "False",
            cp: "False",
            e_r: "False",
        }
    }
}

impl Display for LrcReq<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchReq<'a> {
    s: &'a str,
    r#type: usize,
    limit: usize,
    total: bool,
    offset: usize,
}

impl<'a> SearchReq<'a> {
    pub(crate) fn new(s: &'a str, options: MetingSearchOptions) -> Self {
        let page = if options.page == 0 { 1 } else { options.page };
        Self {
            s,
            r#type: options.r#type,
            limit: options.limit,
            total: true,
            offset: (page - 1) * options.limit,
        }
    }
}

impl Display for SearchReq<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug)]
pub enum ReqError {
    Limit(AcquireError),
    Req(reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct Netease {
    client: Client,
    counter: Arc<Semaphore>,
}

#[cfg(feature = "random-ip")]
pub struct IpStr(String);

#[cfg(feature = "random-ip")]
impl IpStr {
    pub fn random_chinese_ip() -> Self {
        IpStr::from(rand::rng().random_range(1884815360..1884890111))
    }
}

#[cfg(feature = "random-ip")]
impl AsRef<str> for IpStr {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "random-ip")]
impl From<IpStr> for String {
    fn from(ip: IpStr) -> Self {
        ip.0
    }
}

#[cfg(feature = "random-ip")]
impl From<u32> for IpStr {
    fn from(ip: u32) -> Self {
        let octets = [
            (ip >> 24) as u8,
            (ip >> 16) as u8,
            (ip >> 8) as u8,
            ip as u8,
        ];
        Self(format!(
            "{}.{}.{}.{}",
            octets[0], octets[1], octets[2], octets[3]
        ))
    }
}

#[cfg(all(test, feature = "random-ip"))]
mod test_ip_str {
    use crate::netease::IpStr;

    #[test]
    fn test_from_u32() {
        let ip = IpStr::from(1884815360);
        assert_eq!(ip.0, "112.88.0.0");
    }
}

impl Netease {
    pub fn new(counter: Arc<Semaphore>) -> Netease {
        let headers = HeaderMap::new().change_self(|hm|{
            hm.append("Referer" ,HeaderValue::from_static( "https://music.163.com/"));
            hm.append("Cookie" ,HeaderValue::from_static("appver=8.2.30; os=iPhone OS; osver=15.0; EVNSM=1.0.0; buildver=2206; channel=distribution; machineid=iPhone13.3"));
            hm.append("User-Agent" ,HeaderValue::from_static("Mozilla/5.0 (iPhone; CPU iPhone OS 15_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 CloudMusic/0.1.1 NeteaseMusic/8.2.30"));
            hm.append("Accept" , HeaderValue::from_static("*/*"));
            hm.append("Accept-Language" , HeaderValue::from_static("zh-CN,zh;q=0.8,gl;q=0.6,zh-TW;q=0.4"));
            hm.append("Connection" , HeaderValue::from_static("keep-alive"));
            hm.append("Content-Type" , HeaderValue::from_static("application/x-www-form-urlencoded"));
        });
        let client = unsafe {
            ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap_unchecked()
        };
        Self { client, counter }
    }

    pub async fn exec<Output: for<'a> Deserialize<'a>>(
        &self,
        url: &str,
        data: WeapiEncoder,
    ) -> Result<Output, ReqError> {
        let _limit = self.counter.acquire().await.map_err(ReqError::Limit)?;
        self.client
            .post(url)
            .form(&data)
            .then(|req| {
                #[cfg(feature = "random-ip")]
                return req.header("X-Real-IP", IpStr::random_chinese_ip().as_ref());
                #[cfg(not(feature = "random-ip"))]
                return req;
            })
            .send()
            .await
            .map_err(ReqError::Req)?
            .json()
            .await
            .map_err(ReqError::Req)
    }
}

const GET_ID_NAME_PIC_ARTIST_ERR_MSG: &str = "
.id as u64
| .name as str
| .al.pic_str as str / .al.pic as u64
| .ar as array
";

/// # 获取 songs 对象的 id、名称、图片 id、艺术家（们）
///
/// ## None:
///
/// - .id as u64
/// - .name as str
/// - .ar as array
fn get_id_name_artist(input: &Value) -> Option<(String, String, String)> {
    let id = input.get("id")?.as_u64()?.to_string();
    let name = input.get("name")?.as_str()?.to_string();
    let artist = input
        .get("ar")?
        .as_array()?
        .iter()
        .filter_map(|x| x.get("name")?.as_str())
        .enumerate()
        .fold(String::new(), |mut acc, (index, now)| {
            if index != 0 {
                let _ = write!(acc, "/{now}");
                return acc;
            }
            now.to_string()
        });
    Some((id, name, artist))
}

const PLAYLIST_URL: &str = "https://music.163.com/weapi/v6/playlist/detail";
const SONG_INFO_URL: &str = "https://music.163.com/weapi/v3/song/detail";
const SONG_URL: &str = "https://music.163.com/weapi/song/enhance/player/url";
const LRC_URL: &str = "https://music.163.com/weapi/song/lyric";
const SEARCH_URL: &str = "https://music.163.com/weapi/cloudsearch/pc";

const MUSIC_QUALITY: u64 = 320 * 1000;
const ITEM_PRE_REQUEST: usize = 512;
const ENCODER_NAME: &str = "netease";

impl MetingApi for Netease {
    fn name() -> &'static str {
        "netease"
    }

    async fn url(&self, id: &str) -> Result<String, Error> {
        let data = SongFileReq {
            ids: vec![id.to_string()],
            br: MUSIC_QUALITY,
        }
        .to_string()
        .then(|str| WeapiEncoder::try_from_str(&str))
        .map_err(|e| Error::Encode {
            engine: ENCODER_NAME,
            msg: format!("{e:?}"),
        })?
        .then(|we_data| async move { self.exec::<HashMap<String, Value>>(SONG_URL, we_data).await })
        .await
        .map_err(|e| Error::Remote(format!("{e:?}")))?;

        let json = data
            .get("data")
            .ok_or(Error::NoField("data"))?
            .as_array()
            .ok_or(Error::TypeMismatch {
                target: "array",
                feild: "data",
            })?
            .first()
            .ok_or(Error::None)?;
        json.get("code")
            .ok_or(Error::NoField("code"))?
            .as_u64()
            .ok_or(Error::TypeMismatch {
                feild: "code",
                target: "u64",
            })
            .and_then(|x| match x {
                200 => Ok(()),
                _ => Err(Error::None),
            })?;
        json.get("url")
            .or_else(|| json.get("uf")?.get("url"))
            .ok_or(Error::NoField("json.url / json.uf.url"))?
            .as_str()
            .ok_or(Error::TypeMismatch {
                target: "str",
                feild: "json.url / json.uf.url",
            })?
            .replace("http://", "https://")
            .then(Ok)
    }

    async fn pic(&self, id: &str) -> Result<String, Error> {
        let hash_map = id
            .parse::<u64>()
            .map_err(|_| Error::TypeMismatch {
                target: "u64",
                feild: "<id>",
            })?
            .then(SongItem::new)
            .then(|it| [it])
            .then(|its| serde_json::to_string(&its))
            .unwrap()
            .then(SongReq::new)
            .to_string()
            .then(|str| WeapiEncoder::try_from_str(&str))
            .map_err(|e| Error::Encode {
                engine: ENCODER_NAME,
                msg: format!("{e:?}"),
            })?
            .then(|weapi_data| async move {
                self.exec::<HashMap<String, Value>>(SONG_INFO_URL, weapi_data)
                    .await
            })
            .await
            .map_err(|e| Error::Remote(format!("{e:?}")))?;
        let i = hash_map
            .get("songs")
            .ok_or(Error::NoField("songs"))?
            .as_array()
            .ok_or(Error::TypeMismatch {
                target: "array",
                feild: ".songs",
            })?;
        i.first()
            .map(|item| item.get("al")?.get("picUrl"))
            .and_then(|x| x)
            .ok_or(Error::NoField(".songs.0.al.picUrl"))?
            .as_str()
            .ok_or(Error::TypeMismatch {
                target: "str",
                feild: "songs.0.al.picUrl",
            })?
            .to_string()
            .then(Ok)
    }

    async fn lrc(&self, id: &str) -> Result<String, Error> {
        let json =
            LrcReq::new(id)
                .to_string()
                .then(|req| WeapiEncoder::try_from_str(&req))
                .map_err(|e| Error::Encode {
                    engine: ENCODER_NAME,
                    msg: format!("{e:?}"),
                })?
                .then(|we_data| async move {
                    self.exec::<HashMap<String, Value>>(LRC_URL, we_data).await
                })
                .await
                .map_err(|e| Error::Remote(format!("{e:?}")))?;
        json.get("lrc")
            .and_then(|lrc| lrc.get("lyric")?.as_str())
            .unwrap_or("[00:00.00]暂无歌词")
            .to_string()
            .then(Ok)
    }

    async fn song(
        &self,
        id: &str,
        pic: impl Fn(&str) -> String + Send,
        lrc: impl Fn(&str) -> String + Send,
        url: impl Fn(&str) -> String + Send,
    ) -> Result<MetingSong, Error> {
        let json = id
            .parse::<u64>()
            .map_err(|_| Error::TypeMismatch {
                feild: "<id>",
                target: "u64",
            })?
            .then(SongItem::new)
            .then(|it| [it])
            .then(|its| serde_json::to_string(&its))
            .unwrap()
            .then(SongReq::new)
            .to_string()
            .then(|str| WeapiEncoder::try_from_str(&str))
            .map_err(|e| Error::Encode {
                engine: ENCODER_NAME,
                msg: format!("{e:?}"),
            })?
            .then(|weapi_data| async move {
                self.exec::<HashMap<String, Value>>(SONG_INFO_URL, weapi_data)
                    .await
            })
            .await
            .map_err(|e| Error::Remote(format!("{e:?}")))?;
        let (id, name, artist) = json
            .get("songs")
            .unwrap()
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .then(get_id_name_artist)
            .ok_or(Error::NoField(GET_ID_NAME_PIC_ARTIST_ERR_MSG))?;
        MetingSong {
            name,
            artist,
            url: url(&id),
            pic: pic(&id),
            lrc: lrc(&id),
        }
        .then(Ok)
    }

    async fn playlist(
        &self,
        id: &str,
        retry: u8,
        pic: impl Fn(&str) -> String,
        lrc: impl Fn(&str) -> String,
        url: impl Fn(&str) -> String,
    ) -> Result<Vec<MetingSong>, Error> {
        let data = WeapiEncoder::try_from_str(&Playlist::new(id).to_string()).map_err(|e| {
            Error::Encode {
                engine: ENCODER_NAME,
                msg: format!("{e:?}"),
            }
        })?;
        let (bucket, mut bucket_set) = self
            .exec::<HashMap<String, Value>>(PLAYLIST_URL, data)
            .await
            .map_err(|e| Error::Remote(format!("{e:?}")))?
            .get("playlist")
            .and_then(|playlist| playlist.get("trackIds"))
            .ok_or(Error::NoField(".playlist.trackIds"))?
            .then(|track_ids| track_ids.as_array())
            .ok_or(Error::TypeMismatch {
                feild: ".player.trackIds",
                target: "array",
            })?
            .iter()
            .filter_map(|track_id| track_id.get("id").and_then(|id| id.as_u64()))
            .map(SongItem::new)
            .enumerate()
            .fold(
                (Vec::new(), Vec::new()),
                |(mut bucket, mut bucket_set), (index, now)| {
                    bucket.push(now);
                    if index % ITEM_PRE_REQUEST == 0 && index != 0 {
                        bucket_set.push(bucket);
                        bucket = Vec::new()
                    }
                    (bucket, bucket_set)
                },
            );
        bucket_set.push(bucket);
        let tasks = bucket_set
            .iter()
            .map(|items| serde_json::to_string(items).unwrap())
            .map(|bucket| SongReq::new(bucket).to_string())
            .filter_map(|song_req| WeapiEncoder::try_from_str(&song_req).ok())
            .map(|we_data| {
                crate::retry(
                    retry,
                    (Arc::new(we_data), Arc::new(self.clone())),
                    |(we_data, this)| async move {
                        this.exec::<HashMap<String, Value>>(SONG_INFO_URL, we_data.as_ref().clone())
                            .await
                    },
                    |_| (),
                )
            })
            .map(|task| tokio::spawn(task));
        let mut outputs = Vec::with_capacity(ITEM_PRE_REQUEST);
        for task in tasks {
            let Ok(Ok(json)) = task.await else {
                continue;
            };

            json.get("songs")
                .ok_or(Error::NoField("<song-detal>.songs"))?
                .as_array()
                .ok_or(Error::TypeMismatch {
                    feild: "<song-detal>.songs",
                    target: "array",
                })?
                .iter()
                .filter_map(get_id_name_artist)
                .map(|(id, name, artist)| MetingSong {
                    name,
                    artist,
                    url: url(&id),
                    pic: pic(&id),
                    lrc: lrc(&id),
                })
                .for_each(|song| outputs.push(song));
        }
        Ok(outputs)
    }

    async fn search(
        &self,
        keyword: &str,
        option: MetingSearchOptions,
        pic: impl Fn(&str) -> String,
        lrc: impl Fn(&str) -> String,
        url: impl Fn(&str) -> String,
    ) -> Result<Vec<MetingSong>, Error> {
        SearchReq::new(keyword, option)
            .to_string()
            .then(|req| WeapiEncoder::try_from_str(&req))
            .map_err(|e| Error::Encode {
                engine: ENCODER_NAME,
                msg: format!("{e:?}"),
            })?
            .then(|we_data| async move {
                self.exec::<HashMap<String, Value>>(SEARCH_URL, we_data)
                    .await
            })
            .await
            .map_err(|e| Error::Server(format!("{e:?}")))?
            .get("result")
            .and_then(|result| result.get("songs"))
            .ok_or(Error::NoField(".result.songs"))?
            .as_array()
            .ok_or(Error::TypeMismatch {
                feild: ".result.songs",
                target: "array",
            })?
            .iter()
            .filter_map(get_id_name_artist)
            .map(|(id, name, artist)| MetingSong {
                name,
                artist,
                url: url(&id),
                pic: pic(&id),
                lrc: lrc(&id),
            })
            .collect::<Vec<MetingSong>>()
            .then(Ok)
    }
}
