use crate::connection::Peer;

pub struct DHTNode {
    external_nodes: Vec<String>,
}

impl DHTNode {
    pub fn new(trackers: Vec<String>) -> Self {
        let mut external_nodes = trackers;
        if external_nodes.is_empty() {
            println!("No nodes found in DHT, adding default nodes");
            external_nodes = Self::get_default_nodes();
        }

        DHTNode { external_nodes }
    }

    pub fn get_peers(&self) -> Vec<Peer> {
        println!("Found {} nodes in DHT", self.external_nodes.len());
        todo!()
    }

    fn get_default_nodes() -> Vec<String> {
        vec![
            "router.bittorrent.com:6881".to_string(),
            "dht.transmissionbt.com:6881".to_string(),
            "router.utorrent.com:6881".to_string(),
        ]
    }
}
