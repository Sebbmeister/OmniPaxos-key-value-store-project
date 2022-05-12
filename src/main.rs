//Imports
//OmniPaxos library
use omnipaxos_core::{
    sequence_paxos::{SequencePaxos, SequencePaxosConfig},
    storage::{memory_storage::MemoryStorage},
    ballot_leader_election::{BallotLeaderElection, BLEConfig, messages::BLEMessage},
    messages::Message,
    util::LogEntry::{self, Decided},
};
//Tokio - used for network stuff
use tokio::{
    net::{TcpListener, TcpStream},
    io::{self, AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};
//StructOpt - used for getting input from the command line
use structopt::StructOpt;
//Serde - used for serializing (turning into bytes) and deserializing messages
use serde::{Serialize, Deserialize};
//Used for timers
use std::{thread, time};    

//Structs for the nodes and the key-value pairs
#[derive(Debug, Serialize, Deserialize, StructOpt)]
struct Node {
    #[structopt(long)]
    pid: u64,
    #[structopt(long)]
    peers: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: u64,
}

#[tokio::main]
async fn main() {
    //Initialize the node itself
    let node = Node::from_args();    
    let node_number = node.pid;
    let peers = node.peers;
    println!("Initializing node {} with peers {:?}", node_number, peers);

    //The node needs to inform the client of its number of peers
    //The client uses this to determine how many nodes there are and how to divide the key-value pairs between them
    //Get the number
    let number_of_peers: u64 = peers.len().try_into().unwrap();
    //Connect to the client
    let client_stream = TcpStream::connect(format!("127.0.0.1:64000")).await.unwrap();
    let (client_reader, mut client_writer) = tokio::io::split(client_stream);
    let peers_message: Vec<u8> = bincode::serialize(&number_of_peers).unwrap();
    client_writer.write_all(&peers_message).await.unwrap();
    println!("Informed the client of number of peers");

    //Initialize mpsc channels
    //Channels used for BallotLeaderElection
    let (sender_outgoingble, receiver_ble) = mpsc::channel(32);
    let sender_bletimer = sender_outgoingble.clone();
    let sender_blenet = sender_outgoingble.clone();

    //Channels used for SequencePaxos
    let (sender_blehandler, receiver_sp) = mpsc::channel(32);
    let sender_outgoingsp = sender_blehandler.clone();
    let sender_cmdlisten = sender_blehandler.clone();
    let sender_reads = sender_blehandler.clone();
    
    //Configure BallotLeaderElection and SequencePaxos
    let mut ble_config = BLEConfig::default();
    let mut sp_config = SequencePaxosConfig::default();

    ble_config.set_pid(node_number);
    ble_config.set_peers(peers.to_vec());
    ble_config.set_hb_delay(20);
    sp_config.set_configuration_id(node_number.try_into().unwrap());
    sp_config.set_pid(node_number);
    sp_config.set_peers(peers.to_vec());
    
    let ble = BallotLeaderElection::with(ble_config);
    let storage = MemoryStorage::<KeyValue, ()>::default(); 
    let sp = SequencePaxos::with(sp_config, storage);

    //Spawn threads 
    tokio::spawn(async move {
        ble_network_communication(sender_blenet, &node_number).await;
    });
    tokio::spawn(async move {
        ble_periodic_timer(sender_bletimer).await;
    });
    tokio::spawn(async move {
        periodic_send_messages(sender_outgoingsp, sender_outgoingble).await;
    });
    tokio::spawn(async move {
        input_reader(sender_cmdlisten, &node_number).await;
    });
    tokio::spawn(async move {
        handle_ble_messages(ble, receiver_ble, sender_blehandler).await;
    });
    tokio::spawn(async move {
        handle_sp_messages(sp, receiver_sp).await;
    });
    
    //Set up connection
    let mut address: String = "127.0.0.1:".to_owned();
    let node_port: u64 = 50000 + node_number;
    address.push_str(&node_port.to_string().to_owned()); 
    let read_listener = TcpListener::bind(address).await.unwrap();
    
    //Reads have to be handled periodically
    loop {
        let (socket, _) = read_listener.accept().await.unwrap();
        let sender_x = sender_reads.clone();
        tokio::spawn(async move {
            read_handler(socket, node_number, sender_x).await;
        });
    }
}

//The ble_network_communication function listens for ble network activity
async fn ble_network_communication(sender: mpsc::Sender<(&str, Vec<u8>)>, pid: &u64) {
    println!("Listening for BLE network activity");
    //Connect to the right address
    let mut address: String = "127.0.0.1:".to_owned();
    let port_for_node: u64 = 60000 + pid;
    address.push_str(&port_for_node.to_string().to_owned()); 
    
    let stream = TcpListener::bind(address).await.unwrap();
    
    loop {
        let (connection, _) = stream.accept().await.unwrap();    
        let (mut connection_reader, _) = io::split(connection);
        let mut buffer = vec![1; 128];
        loop {
            let n = connection_reader.read(&mut buffer).await.unwrap();
            //Error handling - wrong message size
            if n == 0 || n == 127 {break;}
            //Send handling message
            sender.send(("handle_ble", (&buffer[..n]).to_vec())).await.unwrap();
        }
    }
}

//The ble_periodic_timer function handles ble time checks; sets off the leader handling process for ble in a set interval
async fn ble_periodic_timer(sender: mpsc::Sender<(&str, Vec<u8>)>) {
    loop {
        //20 millisecond timer since this time is used in other portions of the code
        thread::sleep(time::Duration::from_millis(20));
        sender.send(("leader_ble", vec![])).await.unwrap();
    }
}
 
//The periodic_send_messages function periodically checksoutgoing messages and sends them all in a list
async fn periodic_send_messages(sp_sender: mpsc::Sender<(&str, Vec<u8>)>, ble_sender: mpsc::Sender<(&str, Vec<u8>)>) {
    // periodically check outgoing messages and send all in list
    loop {
        thread::sleep(time::Duration::from_millis(1));
        ble_sender.send(("outgoing", vec![])).await.unwrap();
        sp_sender.send(("outgoing", vec![])).await.unwrap();
    }
}

//The read_handler function sends incoming messages to SequencePaxos handle
async fn read_handler(socket_to_read: TcpStream, _id: u64, sender: mpsc::Sender<(&str, Vec<u8>)>) {
    //Establish connection
    let (mut connection, _) = io::split(socket_to_read);
    let mut buffer = vec![1; 128];
    //Loop through messages
    loop {
        let n = connection.read(&mut buffer).await.unwrap();
        //Error handling - wrong message size
        if n == 0 || n == 127 {break;}
        sender.send(("handle_sp", (&buffer[..n]).to_vec())).await.unwrap();
    }
}

// listens for read and write commands from terminal
async fn input_reader(sender: mpsc::Sender<(&str, Vec<u8>)>, node_id: &u64) {
    //Connect to the right address (64500 + the id of the node)
    let mut address: String = "127.0.0.1:".to_owned();
    let node_port: u64 = 64500 + node_id;
    address.push_str(&node_port.to_string().to_owned()); 
    let address_listener = TcpListener::bind(address).await.unwrap();

    //Loop through
    loop {
        let (connection, _) = address_listener.accept().await.unwrap();
        let (mut connection_reader, _) = io::split(connection);
        let mut buffer = vec![1; 128];
        loop {
            let n = connection_reader.read(&mut buffer).await.unwrap();
            //Error handling - wrong message size
            if n == 0 || n == 127 {break;}
            //Deserialize the message
            let deserialized_message: String = bincode::deserialize(&buffer[..n]).unwrap();
            let message_vector:Vec<&str> = deserialized_message.split(" ").collect();

            match message_vector[0] {
                "put" => {
                    let kv = KeyValue{key: String::from(message_vector[1].to_string()), value: message_vector[2].trim().parse().expect("Error: The value should be a number")};
                    sender.send(("put", bincode::serialize(&kv).unwrap())).await.unwrap();
                },
                "get" => {
                    // send string of key to read
                    sender.send(("get", bincode::serialize(&String::from(message_vector[1])).unwrap())).await.unwrap();
                },
                cmd => println!("Error: Received an unknown command"),
    
            }
        }
    }
}

//The handle_ble_messages function handles messages related to the BallotLeaderElection functionality
async fn handle_ble_messages(mut ble: BallotLeaderElection, mut receiver: mpsc::Receiver<(&str, Vec<u8>)>, sender: mpsc::Sender<(&str, Vec<u8>)>) {
    //Go through received messages
    while let Some(action) = receiver.recv().await {
        //Match messages
        match (action.0, action.1) {
            //The leader message is a two-step message since it requires both a ble tick and a "handle leader" in SequencePaxos
            ("leader_ble", ..) => {
                //BLE tick
                if let Some(leader) = ble.tick() {
                    //Re-serialize the message
                    let encrypted_message: Vec<u8> = bincode::serialize(&leader).unwrap();
                    //Send on to SequencePaxos
                    sender.send(("sp_leader", encrypted_message)).await.unwrap();
                }
            },
            //BLE handle so that all messages are handled correctly
            ("handle_ble", encrypted_message) => {
                let deserialized_message: BLEMessage = bincode::deserialize(&encrypted_message).unwrap();
                ble.handle(deserialized_message);
            },
            //Send the outgoing messages
            ("outgoing", ..) => {
                //Loop through outgoing messages
                for outgoing_message in ble.get_outgoing_msgs() {
                    //Get receiver
                    let receiver = outgoing_message.to;
                    //Connect to the correct address
                    match TcpStream::connect(format!("127.0.0.1:{}", 60000 + receiver)).await {
                        Err(_) => println!("ERROR: Bad connection - retrying next round"),
                        Ok(stream) => {
                            //Get writer for the connection
                            let (_reader, mut writer) = io::split(stream);
                            //Serialize and send the messages
                            let encrypted_message: Vec<u8> = bincode::serialize(&outgoing_message).unwrap();
                            writer.write_all(&encrypted_message).await.unwrap();
                        },
                    }
                }
            },
            _ => {
                //If we get an unsupported message
                println!("Received an unknown message type!");
            }
        }
    }
}

//The handle_sp_messages function handles messages related to the SequencePaxos functionality
async fn handle_sp_messages(mut sp: SequencePaxos<KeyValue, (), MemoryStorage<KeyValue, ()>>, mut receiver: mpsc::Receiver<(&str, Vec<u8>)>) {
    //Go through received messages
    while let Some(action) = receiver.recv().await {
        //Match messages
        match (action.0, action.1) {
            //Handle leader - this message is received from the ble handling function
            ("sp_leader", encrypted_message) => {
                sp.handle_leader(bincode::deserialize(&encrypted_message).unwrap());
            },
            //SP handle so that all messages are handled correctly
            ("handle_sp", encrypted_message) => {
                let deserialized_message: Message<KeyValue, ()> = bincode::deserialize(&encrypted_message).unwrap();
                sp.handle(deserialized_message);
            },
            //Send the outgoing messages - essentially the same as for BLE
            ("outgoing", ..) => {
                //Loop through outgoing messages
                for outgoing_message in sp.get_outgoing_msgs() {
                    //Connect to the correct address
                    let receiver = outgoing_message.to;
                    match TcpStream::connect(format!("127.0.0.1:{}", 50000 + receiver)).await {
                        Err(_) => println!("ERROR: Bad connection - retrying next round"),
                        Ok(stream) => {
                            //Get writer for the connection
                            let (_reader, mut writer) = io::split(stream);
                            //Serialize and send the messages
                            let encrypted_message: Vec<u8> = bincode::serialize(&outgoing_message).unwrap();
                            writer.write_all(&encrypted_message).await.unwrap();
                        },
                    }
                }
            }
            //Put adds a key-value pair to the key-value store through using SequencePaxos append
            ("put", encrypted_keyvalue) => {
                println!("Adding key-value pair into the key-value store");
                let keyvalue_to_add: KeyValue = bincode::deserialize(&encrypted_keyvalue).unwrap();
                sp.append(keyvalue_to_add).expect("ERROR: Could not add key-value pair into the key-value store");

            },
            //Get searches the key-value store using the decided list of SequencePaxos
            ("get", encrypted_key) => {
                //Get the key to search for in the key-value store
                let key: String = bincode::deserialize(&encrypted_key).unwrap();
                //Get all of the decided values from SequencPaxos
                let decided_values: Option<Vec<LogEntry<KeyValue, ()>>> = sp.read_decided_suffix(0);
                
                //let decided_values: Option<Vec<LogEntry<KeyValue, ()>>> = sp.read_entries(0..100);
                
                //Connect to the client - this needs to be created twice to avoid rust complaining about "moving"
                let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 64500)).await.unwrap();
                let (_client_reader, mut client_writer) = tokio::io::split(client_stream);
                //Examine the decided values
                match decided_values {
                    Some(decided_vector) => {
                        //For debugging purposes and testing in case something goes wrong
                        //println!("Vector of decided values {:?}", vec.to_vec());
                        //Get the vector itself
                        let new_vector: Vec<LogEntry<'_, KeyValue, ()>> = decided_vector.to_vec();
                        //Connect to the client (again)
                        let client_stream2 = TcpStream::connect(format!("127.0.0.1:{}", 64500)).await.unwrap();
                        let (_client_reader2, mut client_writer2) = tokio::io::split(client_stream2);  
                        //Go through the decided vector and look for key-value pair
                        let mut index = new_vector.len()-1;
                        let mut response = String::new(); //Prepare response string
                        let mut found_kv_pair = false; //Boolean to indicate whether what we are looking for has been found or not
                        loop {
                            match new_vector[index] {
                                Decided(kv) => {
                                    //If we find what we are looking for
                                    if kv.key == key {
                                        //Add both the key and the value to the reponse message
                                        response.push_str(format!("{}", kv.key).as_str());
                                        response.push_str(format!(" {}", kv.value).as_str());
                                        //Set "found_kv_pair" to true
                                        found_kv_pair = true;
                                    }
                                },
                                _ => println!("Continuing to look for key-value pair"),
                            };
                            //If we have looped through the entire vector and not found anything or if we found what we wanted - we are done
                            if index == 0 || found_kv_pair {
                                break;
                            }
                            //Decrement the index we are looking at
                            index-=1;
                        }
                        //If nothing was found response is still an empty string
                        if response.len() <= 1 {
                            //Set it to "not found"
                            response = "not found".to_string();
                        }
                        //Send the message to the client (key-value pair in case something was found, "not found" otherwise)
                        let encrypted_message: Vec<u8> = bincode::serialize(&response).unwrap();
                        client_writer2.write_all(&encrypted_message).await.unwrap();
                    },
                    _ => {
                        //If we do not get a vector that means that nothing is in the key-value store, so we need to throw an error
                        println!("ERROR: Could not get KeyValue pair from Sequence Paxos");
                        //This needs to be sent to the client so that it does not get an error and informs the user that the key was not found
                        let notfound_message: Vec<u8> = bincode::serialize("not found").unwrap();
                        client_writer.write_all(&notfound_message).await.unwrap();
                    },
                }
            },
            _ => {
                //If we get an unsupported message
                println!("Received an unknown message type!");
            }
        }
    }                
}