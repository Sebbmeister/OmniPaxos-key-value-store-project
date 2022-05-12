//Imports
//Tokio - used for network stuff
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
//Serde - used for serializing (turning into bytes) and deserializing messages
use serde::{Serialize, Deserialize};

#[tokio::main]
async fn main() {
    //Create mpsc channels for communication
    //Receiver will handle incoming messages, sender_peers will send peers messages and sender_messages will send other messages
    let (sender_peers, receiver) = mpsc::channel(32);
    let sender_messages = sender_peers.clone();

    //Spawn threads
    tokio::spawn(async move {
        get_peers(sender_peers).await;
    }); 
    tokio::spawn(async move {
        give_results().await;
    });
    tokio::spawn(async move {
        message_receiver(receiver).await;
    });
    
    //Std:io is required for the read_line method; needs to be imported here in order to not conflict with Tokio
    use std::io;
    //Loop through and read input from the command line of the client
    loop {
        //Get the input command
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect(" -> ERROR: Could not read the input");
        //Vectorize the input - check the first word to determine if it is a get or a put message
        let input_vector:Vec<&str> = input.split(" ").collect();
        if input_vector[0] == "get" {sender_messages.send(("get", bincode::serialize(&input).unwrap())).await.unwrap();}
        else if input_vector[0] == "put" {sender_messages.send(("put", bincode::serialize(&input).unwrap())).await.unwrap();}
        else{
            //If it is not a put or a get
            println!(" -> ERROR: Unknown command");
        }
    }
}

//The get_peers functions receives messages from the nodes on their number of peers and sends the information
//to the primary message-handling function of the client
async fn get_peers(sender: mpsc::Sender<(&str, Vec<u8>)>) {
    //Listen on the set "peers" address
    let address = TcpListener::bind("127.0.0.1:64000").await.unwrap();

    //Loop through received messages on the address
    loop{
        //Establish connection properly
        let (connection, _) = address.accept().await.unwrap();
        let (mut connection_reader, _) = io::split(connection);
        let mut buffer = [1; 128];

        loop{
            let n = connection_reader.read(&mut buffer).await.unwrap();
            match n {
                //If n = 0 we have not received any messages
                0 => break,
                //If n != 0 we have received a message
                peers => {
                    //Deserialize the number of peers
                    let deserialized_peers: u64 = bincode::deserialize(&buffer[0..peers]).unwrap();
                    //Send it on to the main message-handling function
                    sender.send(("peers", bincode::serialize(&(deserialized_peers + 1)).unwrap())).await.unwrap();
                    break;
                },
            }
        }
    }
} 

//The give_results function outputs the result of a "get" operation
async fn give_results() {
    //Print that the client is read to take commands
    println!("Ready for operations");

    //Establish connection
    let address = TcpListener::bind("127.0.0.1:64500").await.unwrap();
    loop {
        let (connection, _) = address.accept().await.unwrap();
        let (mut connection_reader, _) = io::split(connection);
        let mut buffer = vec![1; 128];

        loop {
            let n = connection_reader.read(&mut buffer).await.unwrap();
            //Error handling - wrong message size
            if n == 0 || n == 127 {break;}
            //Get return message from "get" operation; presumably a key-value pair
            let return_message: String = bincode::deserialize(&buffer[..n]).unwrap();
            //Split up the message so that its different parts can be examined
            let message_vector:Vec<&str> = return_message.split(" ").collect();

            //Error handling - if the "get" was for a key that has not been added
            if message_vector[0] == "not" {println!(" -> ERROR: Key not found in database - try searching for a key that exists");}
            //If the key does exist
            else {println!(" -> Found key-value pair: key {} value {}", message_vector[0], message_vector[1]);}   
        }
    }
}

//The message_receiver function handles the messages sent within the client's code
async fn message_receiver(mut receiver: mpsc::Receiver<(&str, Vec<u8>)>) {
    //Record of the number of peers (i.e. active nodes - 1), default is 0
    let mut number_of_peers: u64 = 0; 
    //Go through messages
    while let Some(action) = receiver.recv().await {
        match (action.0, action.1) {
            //Message from a node with an updated number of peers; update the number set here
            ("peers", updated_number_of_peers) => {
                let deserialized_update: u64 = bincode::deserialize(&updated_number_of_peers).unwrap();
                number_of_peers = deserialized_update;
            },
            //Put or get message
            (_, message) => {
                let mut node = 0;
                let mut deserialized_message: String = bincode::deserialize(&message).unwrap();
                let message_vector:Vec<&str> = deserialized_message.split(" ").collect();

                let mut key: u64 = message_vector[1].trim().parse().expect(" -> ERROR: The key needs to be a number");
                if action.0 == "put" && message_vector.len() == 2 {
                    println!(" -> ERROR: Put message requires a value");
                }
                else if action.0 == "get" || message_vector.len() == 3 {
                    //Loop to see which node gets the message; each node handles up to five keys, other than the last one which handles everything higher
                    //than the key of the second last node
                    for number in 0..key {
                        if number % 5 == 0{
                            node = node + 1;
                        }
                        if number == key || node >= number_of_peers{
                            break;
                        }
                    } 
                    //Connect to the right node
                    let mut address: String = "127.0.0.1:".to_owned();
                    let port_of_node: u64 = 64500 + node;
                    address.push_str(&port_of_node.to_string().to_owned()); 
                    let stream = TcpStream::connect(address).await.unwrap();
                    //Print to the client so that it is possible to see what is going on
                    if action.0 == "put"{
                        println!(" -> Sending put message to node {}", node);
                    }
                    else {
                        println!(" -> Sending get message to node {}", node);
                    }
                    //Send the message
                    let (_reader, mut writer) = tokio::io::split(stream);    
                    let encrypted_message: Vec<u8> = bincode::serialize(&deserialized_message.trim()).unwrap();
                    writer.write_all(&encrypted_message).await.unwrap();
                }
                else{
                    println!(" -> ERROR: Unforeseen error when trying to put or get");
                }
            },
            //If a message type not implemented here is received
            _ => {
                println!(" -> ERROR: Received an unknown message type");
            },
        }
    }
}