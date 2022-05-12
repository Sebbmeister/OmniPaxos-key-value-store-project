::Executes commands in a serial order
::Start a new command prompt for the client using the run_client batch file
start cmd /k run_client.bat %this_dir%

::Timeout so that the client has time to get going (nodes require it to be running)
timeout /t 4

::Launch four nodes
start cmd /k cargo run --bin omnipaxos-key-value-store -- --pid 1 --peers 2 3 4 5
start cmd /k cargo run --bin omnipaxos-key-value-store -- --pid 2 --peers 3 1 4 5
start cmd /k cargo run --bin omnipaxos-key-value-store -- --pid 3 --peers 1 2 4 5
start cmd /k cargo run --bin omnipaxos-key-value-store -- --pid 4 --peers 1 2 3 5
start cmd /k cargo run --bin omnipaxos-key-value-store -- --pid 5 --peers 1 2 3 4

::Clear the command prompt output
cls

