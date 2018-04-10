mod cypher_view;
mod neo4j_view;

pub use self::{cypher_view::CypherView, neo4j_view::Neo4JView};

use std::{collections::HashMap, str::FromStr};

use neo4j::{Node, Value};

use data::{node_types::{EditInit, EditSession, EnumNode, File, FileInit, Pipe, PipeInit, Process,
                        ProcessInit, Socket, SocketClass, SocketInit},
           Enumerable,
           Generable,
           HasID,
           HasUUID,
           NodeID};

use uuid::Uuid5;

impl From<Uuid5> for Value {
    fn from(val: Uuid5) -> Self {
        Value::String(val.to_string())
    }
}

pub trait IntoUUID {
    fn into_uuid5(self) -> Option<Uuid5>;
}

impl IntoUUID for Value {
    fn into_uuid5(self) -> Option<Uuid5> {
        match self {
            Value::String(s) => Uuid5::from_str(&s).ok(),
            _ => None,
        }
    }
}

impl From<NodeID> for Value {
    fn from(val: NodeID) -> Self {
        Value::Integer(val.inner())
    }
}

pub trait IntoNodeID {
    fn into_nodeid(self) -> Option<NodeID>;
}

impl IntoNodeID for Value {
    fn into_nodeid(self) -> Option<NodeID> {
        match self {
            Value::Integer(i) => Some(NodeID::new(i)),
            _ => None,
        }
    }
}

pub trait ToDB: HasID + HasUUID {
    fn get_labels(&self) -> Vec<&'static str>;
    fn get_props(&self) -> HashMap<&'static str, Value>;
    fn to_db(&self) -> (NodeID, Vec<&'static str>, HashMap<&'static str, Value>) {
        let mut props = self.get_props();
        props.insert("db_id", self.get_db_id().into());
        props.insert("uuid", self.get_uuid().into());
        (self.get_db_id(), self.get_labels(), props)
    }
}

impl ToDB for EnumNode {
    fn get_labels(&self) -> Vec<&'static str> {
        match *self {
            EnumNode::EditSession(_) => vec!["Node", "EditSession"],
            EnumNode::File(_) => vec!["Node", "File"],
            EnumNode::Pipe(_) => vec!["Node", "Pipe"],
            EnumNode::Proc(_) => vec!["Node", "Process"],
            EnumNode::Socket(_) => vec!["Node", "Socket"],
        }
    }
    fn get_props(&self) -> HashMap<&'static str, Value> {
        match *self {
            EnumNode::EditSession(ref e) => hashmap!("name"  => Value::from(e.name.clone())),
            EnumNode::File(ref f) => hashmap!("name"  => Value::from(f.name.clone())),
            EnumNode::Pipe(ref p) => hashmap!("fd"    => Value::from(p.fd)),
            EnumNode::Proc(ref p) => hashmap!("cmdline" => Value::from(p.cmdline.clone()),
                                              "pid"     => Value::from(p.pid),
                                              "thin"    => Value::from(p.thin)),
            EnumNode::Socket(ref s) => hashmap!("class"  => Value::from(s.class as i64),
                                                "path" => Value::from(s.path.clone()),
                                                "ip" => Value::from(s.ip.clone()),
                                                "port" => Value::from(s.port)),
        }
    }
}

pub trait FromDB {
    fn from_value(val: Value) -> Result<Self, &'static str>
    where
        Self: Sized;
}

impl FromDB for EnumNode {
    fn from_value(val: Value) -> Result<Self, &'static str> {
        let mut g = Node::from_value(val)?;

        let id = g.props
            .remove("db_id")
            .and_then(Value::into_nodeid)
            .ok_or("db_id property is missing or not an Integer")?;
        let uuid = g.props
            .remove("uuid")
            .and_then(Value::into_uuid5)
            .ok_or("uuid property is missing or not a UUID5")?;

        if g.labs.contains(&String::from("Process")) {
            Ok(Process::new(id, uuid, Some(g.into_init()?)).enumerate())
        } else if g.labs.contains(&String::from("File")) {
            Ok(File::new(id, uuid, Some(g.into_init()?)).enumerate())
        } else if g.labs.contains(&String::from("EditSession")) {
            Ok(EditSession::new(id, uuid, Some(g.into_init()?)).enumerate())
        } else if g.labs.contains(&String::from("Socket")) {
            Ok(Socket::new(id, uuid, Some(g.into_init()?)).enumerate())
        } else if g.labs.contains(&String::from("Pipe")) {
            Ok(Pipe::new(id, uuid, Some(g.into_init()?)).enumerate())
        } else {
            Err("Node doesn't match any known type.")
        }
    }
}

trait IntoInit<T> {
    fn into_init(self) -> Result<T, &'static str>;
}

impl IntoInit<FileInit> for Node {
    fn into_init(mut self) -> Result<FileInit, &'static str> {
        Ok(FileInit {
            name: self.props
                .remove("name")
                .and_then(Value::into_string)
                .ok_or("name property is missing or not a string")?,
        })
    }
}

impl IntoInit<EditInit> for Node {
    fn into_init(mut self) -> Result<EditInit, &'static str> {
        Ok(EditInit {
            name: self.props
                .remove("name")
                .and_then(Value::into_string)
                .ok_or("name property is missing or not a string")?,
        })
    }
}

impl IntoInit<PipeInit> for Node {
    fn into_init(mut self) -> Result<PipeInit, &'static str> {
        Ok(PipeInit {
            fd: self.props
                .remove("fd")
                .and_then(Value::into_int)
                .ok_or("fd property is missing or not an Integer")?,
        })
    }
}

impl IntoInit<ProcessInit> for Node {
    fn into_init(mut self) -> Result<ProcessInit, &'static str> {
        Ok(ProcessInit {
            cmdline: self.props
                .remove("cmdline")
                .and_then(Value::into_string)
                .ok_or("cmdline property is missing or not a String")?,
            pid: self.props
                .remove("pid")
                .and_then(Value::into_int)
                .ok_or("pid property is missing or not an Integer")?,
            thin: self.props
                .remove("thin")
                .and_then(Value::into_bool)
                .ok_or("thin property is missing or not a bool")?,
        })
    }
}

impl IntoInit<SocketInit> for Node {
    fn into_init(mut self) -> Result<SocketInit, &'static str> {
        Ok(SocketInit {
            class: self.props
                .remove("class")
                .and_then(Value::into_int)
                .and_then(SocketClass::from_int)
                .ok_or("class property is missing or not an Integer")?,
            path: self.props
                .remove("path")
                .and_then(Value::into_string)
                .ok_or("path property is missing or not a string")?,
            ip: self.props
                .remove("ip")
                .and_then(Value::into_string)
                .ok_or("ip property is missing or not a string")?,
            port: self.props
                .remove("port")
                .and_then(Value::into_int)
                .ok_or("port property is missing or not an Integer")?,
        })
    }
}