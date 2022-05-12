# Replicated key-value store
Final project in the KTH course ID2203 (distributed systems advanced course). This is a replicated key-value store implemented using OmniPaxos ([link](https://github.com/haraldng/omnipaxos)). 

### Instructions
The easiest way to run the key-value store is to use the run_kvstore.bat batch file - simply write `cargo build` and then `run_kvstore.bat` in the command prompt once you've navigated to the correct folder.

The key-value store can also be run manually. After running `cargo build`, the client is started through writing `cargo run --bin client`. *After* starting the client, nodes can be started through the command `cargo run --bin omnipaxos-key-value-store -- --pid [number] --peers [list of peers]`. To start a node with number 4 and connect it to nodes 1, 2 and 3, the user for instance writes `cargo run --bin omnipaxos-key-value-store -- --pid 4 --peers 1 2 3`.

The key-value store supports the commands "put" (which adds key-value pairs to the store) and "get" (which retrieves a value associated with a key asked for by the user). "Put" commands are written `put [key] [value]` (i.e. to add the key-value pair 2, 3: `put 2 3`) and "get" commands are written `get [key]` (i.e. to retrieve the value associated with the key 5: `get 5`).
