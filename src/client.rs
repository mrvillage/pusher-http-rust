use rustc_serialize::{self, json};
use hyper::{Url, Client};
use queryst::parse;
use std::collections::HashMap;

use std::env;

use super::signature::*;
use super::request::*;
use super::request_url::*;
use super::json_structures::*;
use super::QueryParameters;

pub struct Pusher {
  app_id: String,
  key: String,
  secret: String, 
  host: String,
  secure: bool,
  http_client: Client,
}

pub struct PusherBuilder {
  app_id: String,
  key: String,
  secret: String, 
  host: String,
  secure: bool,
  http_client: Client,
}

impl PusherBuilder{
  pub fn host(mut self, host: &str) -> PusherBuilder{
    self.host = host.to_string();
    self
  }

  pub fn secure(mut self, secure: bool) -> PusherBuilder {  
    self.secure = secure;
    self
  }

  pub fn client(mut self, http_client: Client) -> PusherBuilder {
    self.http_client = http_client;
    self
  }

  pub fn finalize(self) -> Pusher {
   Pusher {
      app_id: self.app_id,
      key: self.key,
      secret: self.secret,
      host: self.host,
      secure: self.secure,
      http_client: self.http_client,
    } 
  }

}

impl Pusher{

  pub fn new(app_id: &str, key: &str, secret: &str) -> PusherBuilder {
    let mut http_client = Client::new();

    PusherBuilder {
      app_id: app_id.to_string(),
      key: key.to_string(),
      secret: secret.to_string(),
      host: "api.pusherapp.com".to_string(),
      secure: false,
      http_client: http_client,
    }

  }

  pub fn from_env(key: &str) -> PusherBuilder {
    let url_opt = env::var_os(key).unwrap();
    let os_url = url_opt.to_str();
    let url = os_url.unwrap();
    Pusher::from_url(url)
  }

  pub fn from_url(url: &str) -> PusherBuilder {
    let pusher_url = Url::parse(url).unwrap();

    let key = pusher_url.username().unwrap();
    let secret = pusher_url.password().unwrap();
    let host = pusher_url.host().unwrap();
    let path = pusher_url.path().unwrap();
    let app_id = &path[1];
    let mut secure  = false;

    if pusher_url.scheme == "https" {
      secure = true;
    }

    let mut http_client = Client::new();

    PusherBuilder {
      app_id: app_id.to_string(),
      key: key.to_string(),
      secret: secret.to_string(),
      host: host.to_string(),
      secure: secure,
      http_client: http_client,
    }

  }

  pub fn trigger<Payload : rustc_serialize::Encodable>(&mut self, channel: &str, event: &str, payload: Payload)-> String {
    let channels = vec![channel.to_string()];
    self._trigger(channels, event, payload, None)
  }

  pub fn trigger_exclusive<Payload : rustc_serialize::Encodable>(&mut self, channel: &str, event: &str, payload: Payload, socket_id: &str)-> String {
    let channels = vec![channel.to_string()];
    self._trigger(channels, event, payload, Some(socket_id.to_string()))
  }

  pub fn trigger_multi<Payload : rustc_serialize::Encodable>(&mut self, channels: Vec<&str>, event: &str, payload: Payload)-> String {
    let channel_strings = channels.into_iter().map(|c| c.to_string()).collect();
    self._trigger(channel_strings, event, payload, None)
  }

  pub fn trigger_multi_exclusive<Payload : rustc_serialize::Encodable>(&mut self, channels: Vec<&str>, event: &str, payload: Payload, socket_id: &str)-> String {
    let channel_strings = channels.into_iter().map(|c| c.to_string()).collect();
    self._trigger(channel_strings, event, payload, Some(socket_id.to_string()))
  }

  fn _trigger<Payload : rustc_serialize::Encodable>(&mut self, channels: Vec<String>, event: &str, payload: Payload, socket_id: Option<String>) -> String { 
    let request_url_string = format!("{}://{}/apps/{}/events", self.scheme(), self.host, self.app_id);
    let mut request_url = Url::parse(&request_url_string).unwrap();

    let json_payload = json::encode(&payload).unwrap();

    let raw_body = TriggerEventData{
      name: event.to_string(),
      channels: channels,
      data: json_payload,
      socket_id: socket_id,
    };

    let body = json::encode(&raw_body).unwrap();

    let method = "POST";
    update_request_url(method, &mut request_url, &self.key, &self.secret, timestamp(), Some(&body), None);
    send_request(&mut self.http_client, method, request_url, Some(&body)) // TODO - return buffered events
  }

  pub fn channels(&mut self, params: QueryParameters) -> ChannelList{
    let request_url_string = format!("{}://{}/apps/{}/channels", self.scheme(), self.host, self.app_id);
    let mut request_url = Url::parse(&request_url_string).unwrap();
    let method = "GET";
    update_request_url(method, &mut request_url, &self.key, &self.secret, timestamp(), None, params);
    create_request::<ChannelList>(&mut self.http_client, method, request_url, None)
  }

  fn scheme(&self) -> &str {
    if self.secure {
      "https"
    } else {
      "http"
    }
  }

  pub fn channel(&mut self, channel_name: &str, params: QueryParameters) -> Channel{
    let request_url_string = format!("{}://{}/apps/{}/channels/{}", self.scheme(), self.host, self.app_id, channel_name);
    let mut request_url = Url::parse(&request_url_string).unwrap();
    let method = "GET";
    update_request_url(method, &mut request_url, &self.key, &self.secret, timestamp(), None, params);
    create_request::<Channel>(&mut self.http_client, method, request_url, None)
  }

  pub fn channel_users(&mut self, channel_name : &str) -> ChannelUserList {
    let request_url_string = format!("{}://{}/apps/{}/channels/{}/users", self.scheme(), self.host, self.app_id, channel_name);
    let mut request_url = Url::parse(&request_url_string).unwrap();
    let method = "GET";
    update_request_url(method, &mut request_url, &self.key, &self.secret, timestamp(), None, None);
    create_request::<ChannelUserList>(&mut self.http_client, method, request_url, None)
  }

  pub fn authenticate_private_channel(&self, body: &String) -> String {
    self.authenticate_channel(body, None)
  }

  pub fn authenticate_presence_channel(&self, body: &String, member: &Member) -> String {
    self.authenticate_channel(body, Some(member))
  }

  fn authenticate_channel(&self, body: &String, member: Option<&Member>) -> String {
    let object = parse(body);
    let auth : AuthParams = json::decode(&object.unwrap().to_string()).unwrap();

    let mut auth_map = HashMap::new();
    let channel_name = auth.channel_name;
    let socket_id = auth.socket_id;
    let mut to_sign = format!("{}:{}", socket_id, channel_name);

    if let Some(presence_member) = member {
      let json_member = json::encode(presence_member).unwrap();
      to_sign = format!("{}:{}", to_sign, json_member);
      auth_map.insert("channel_data", json_member);
    }

    create_channel_auth(&mut auth_map, &self.key, &self.secret, &to_sign);
    json::encode(&auth_map).unwrap()
  }

  pub fn webhook(&self, key: &String, signature: &String, body: &str) -> Result<Webhook, &str> {
    if (&self.key == key) && check_signature(signature, &self.secret, body) {
      println!("Checks out");
      println!("{:?}", body);
      let decoded_webhook : Webhook = json::decode(&body[..]).unwrap();
      return Ok(decoded_webhook)
    }
    Err("Invalid webhook")
  }

}
