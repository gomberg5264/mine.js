use rand::{self, rngs::ThreadRng, Rng};

use actix::prelude::*;
use actix_web_actors::ws;

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::thread::current;
use std::time::{Duration, Instant};

use crate::core::world::World;
use crate::libs::types::{Coords2, Coords3, Quaternion};
use crate::models::{
    self,
    messages::{self, message::Type as MessageType},
};
use crate::utils::convert::{map_voxel_to_chunk, map_world_to_voxel};
use crate::utils::json;

use super::models::ChunkProtocol;
use super::registry::Registry;
use super::world::WorldMetrics;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CHUNKING_INTERVAL: Duration = Duration::from_millis(16);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(MessageResponse)]
pub struct ConnectionResult {
    pub id: usize,
    pub metrics: WorldMetrics,
}

#[derive(Message)]
#[rtype(result = "ConnectionResult")]
pub struct Connect {
    pub world_name: String,
    pub addr: Recipient<Message>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Generate {
    pub world: String,
    pub coords: Coords2<i32>,
    pub render_radius: i16,
}

#[derive(MessageResponse)]
pub struct ChunkRequestResult {
    protocol: Option<ChunkProtocol>,
}

#[derive(Message)]
#[rtype(result = "ChunkRequestResult")]
pub struct ChunkRequest {
    pub world: String,
    pub needs_voxels: bool,
    pub coords: Coords2<i32>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    // id of client session
    pub id: usize,
    // Peer message
    pub msg: models::messages::Message,
    // Room name
    pub world: String,
}

// list of available rooms
pub struct ListWorlds;

impl actix::Message for ListWorlds {
    type Result = Vec<String>;
}

#[derive(Debug)]
pub struct WsServer {
    clients: HashMap<usize, Recipient<Message>>,
    worlds: HashMap<String, World>,
    rng: ThreadRng,
}

impl WsServer {
    pub fn new() -> WsServer {
        let mut worlds: HashMap<String, World> = HashMap::new();

        let worlds_json: serde_json::Value =
            serde_json::from_reader(File::open("metadata/worlds.json").unwrap()).unwrap();

        let world_default = &worlds_json["default"];

        let registry = Registry::new();

        for world_json in worlds_json["worlds"].as_array().unwrap() {
            let mut world_json = world_json.clone();
            json::merge(&mut world_json, world_default, false);

            let mut new_world = World::new(world_json, registry.clone());
            new_world.preload();
            worlds.insert(new_world.name.to_owned(), new_world);
        }

        WsServer {
            worlds,
            clients: HashMap::new(),
            rng: rand::thread_rng(),
        }
    }

    pub fn send_message(&self, world: &str, message: &str, skip_id: usize) {
        if let Some(world) = self.worlds.get(world) {
            for (id, recipient) in &world.clients {
                if *id != skip_id {
                    recipient.do_send(Message(message.to_owned())).unwrap();
                }
            }
        }
    }
}

impl Actor for WsServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for WsServer {
    type Result = MessageResult<Connect>;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined");

        // TODO: send join message here.
        self.send_message(&"Main".to_owned(), "Someone joined", 0);

        // register session with random id
        let id = self.rng.gen::<usize>();
        self.clients.insert(id, msg.addr.clone()); // ? NOT SURE IF THIS WORKS

        let world_name = msg.world_name;
        let world = self.worlds.get_mut(&world_name).unwrap();
        world.add_client(id, msg.addr.to_owned());

        MessageResult(ConnectionResult {
            id,
            metrics: world.chunks.metrics.clone(),
        })
    }
}

impl Handler<ListWorlds> for WsServer {
    type Result = MessageResult<ListWorlds>;

    fn handle(&mut self, _: ListWorlds, _: &mut Context<Self>) -> Self::Result {
        let mut worlds = Vec::new();

        for key in self.worlds.keys() {
            worlds.push(key.to_owned());
        }

        MessageResult(worlds)
    }
}

impl Handler<Generate> for WsServer {
    type Result = ();

    fn handle(&mut self, data: Generate, _: &mut Context<Self>) {
        let Generate {
            coords,
            render_radius,
            world,
        } = data;

        let world = self.worlds.get_mut(&world).unwrap();
        world.chunks.generate(coords, render_radius);
    }
}

impl Handler<ChunkRequest> for WsServer {
    type Result = MessageResult<ChunkRequest>;

    fn handle(&mut self, request: ChunkRequest, _: &mut Context<Self>) -> Self::Result {
        let ChunkRequest {
            world,
            coords,
            needs_voxels,
        } = request;

        let world = self.worlds.get_mut(&world).unwrap();

        let chunk = world.chunks.get(&coords);

        if chunk.is_none() {
            return MessageResult(ChunkRequestResult { protocol: None });
        }

        let chunk = chunk.unwrap();

        // TODO: OPTIMIZE THIS? CLONE?
        MessageResult(ChunkRequestResult {
            protocol: Some(chunk.get_protocol(needs_voxels)),
        })
    }
}

impl Handler<Disconnect> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let mut worlds: Vec<String> = Vec::new();

        // remove address
        if self.clients.remove(&msg.id).is_some() {
            // remove session from all rooms
            for world in self.worlds.values_mut() {
                let id = world.clients.remove_entry(&msg.id);
                if id.is_some() {
                    worlds.push(world.name.to_owned());
                }
            }
        }

        for world in worlds {
            self.send_message(&world, "Someone disconnected", 0)
        }
    }
}

#[derive(Debug)]
pub struct WsSession {
    // unique sessions id
    pub id: usize,
    // client must ping at least once per 10 seconds (CLIENT_TIMEOUT)
    // otherwise we drop connection
    pub hb: Instant,
    // joined world
    pub world_name: String,
    // world metrics
    pub metrics: Option<WorldMetrics>,
    // name in world
    pub name: Option<String>,
    // chat server
    pub addr: Addr<WsServer>,
    // position in world
    pub position: Coords3<f32>,
    // rotation in world
    pub rotation: Quaternion,
    // current chunk in world
    pub current_chunk: Option<Coords2<i32>>,
    // requested chunk in world
    pub requested_chunks: VecDeque<Coords2<i32>>,
    // radius of render?
    pub render_radius: i16,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address();
        self.addr
            .send(Connect {
                world_name: self.world_name.to_owned(),
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => {
                        act.id = res.id;
                        act.metrics = Some(res.metrics);
                    }
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);

        self.chunk(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        // TODO: PROTOCOL BUFFER SENDING HERE
        ctx.text(msg.0)
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Binary(bytes) => {
                let message = models::decode_message(&bytes.to_vec()).unwrap();
                self.on_request(message);
            }
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            _ => (),
        }
    }
}

impl WsSession {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Websocket Client heartbeat failed, disconnecting!");

                act.addr.do_send(Disconnect { id: act.id });
                ctx.stop();

                return;
            }

            ctx.ping(b"");
        });
    }

    fn chunk(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(CHUNKING_INTERVAL, |act, ctx| {
            let requested_chunk = act.requested_chunks.pop_front();

            if let Some(coords) = requested_chunk {
                act.addr
                    .send(ChunkRequest {
                        needs_voxels: true,
                        coords: coords.clone(),
                        world: act.world_name.to_owned(),
                    })
                    .into_actor(act)
                    .then(|res, act, ctx| {
                        match res {
                            Ok(ChunkRequestResult { protocol }) => {
                                if protocol.is_none() {
                                    act.requested_chunks.push_back(coords);
                                } else {
                                    let protocol = protocol.unwrap();

                                    println!("Meshes received for {:?}", coords);
                                }
                            }
                            _ => ctx.stop(),
                        }
                        fut::ready(())
                    })
                    .wait(ctx);
            }
        });
    }

    fn on_request(&mut self, message: messages::Message) {
        let msg_type = messages::Message::r#type(&message);

        match msg_type {
            MessageType::Request => {
                let json = message.parse_json().unwrap();

                let cx = json["x"].as_i64().unwrap() as i32;
                let cz = json["z"].as_i64().unwrap() as i32;

                self.requested_chunks.push_back(Coords2(cx, cz));
            }
            MessageType::Config => {}
            MessageType::Update => {}
            MessageType::Peer => {
                let messages::Peer {
                    name,
                    px,
                    py,
                    pz,
                    qx,
                    qy,
                    qz,
                    qw,
                    ..
                } = &message.peers[0];

                // means this player just joined.
                if self.name.is_none() {
                    // TODO: broadcast "joined the game" message
                }

                self.name = Some(name.to_owned());
                self.position = Coords3(*px, *py, *pz);
                self.rotation = Quaternion(*qx, *qy, *qz, *qw);

                let WorldMetrics {
                    chunk_size,
                    dimension,
                    ..
                } = self.metrics.as_ref().unwrap();

                let current_chunk = self.current_chunk.as_ref();
                let new_chunk = map_voxel_to_chunk(
                    &map_world_to_voxel(&self.position, *dimension),
                    *chunk_size,
                );

                if current_chunk.is_none()
                    || current_chunk.unwrap().0 != new_chunk.0
                    || current_chunk.unwrap().1 != new_chunk.1
                {
                    self.current_chunk = Some(new_chunk.clone());
                    self.addr.do_send(Generate {
                        coords: new_chunk,
                        render_radius: self.render_radius,
                        world: self.world_name.to_owned(),
                    });
                }
            }
            MessageType::Message => {}
            MessageType::Init => {
                println!("INIT?")
            }
            _ => {}
        }
        // println!("TYPE OF MESSAGE: {}", message.r#type);

        // if !message.json.is_empty() {
        //     let json = message
        //         .parse_json()
        //         .expect("DAMN WTF ERROR IN PARSING JSON");
        //     println!("{:?}", json);
        // }
    }
}
