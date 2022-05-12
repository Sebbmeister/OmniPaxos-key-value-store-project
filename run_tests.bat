::Old experimental test file - idea was to read the log to ensure accurate behaviour

::Write to (or create if it does not exist) the file test_results.txt
echo Test results: > test_results.txt

::Open the logs folder to read the omnipaxos logs
cd logs

::Check if node 1 got connected to node 2 and 3
::Findstr searches for a string in a file; if fails it goes to :node1fail
findstr /c:"Heartbeat request from 2" paxos_1.log || goto :node1fail
findstr /c:"Heartbeat request from 3" paxos_1.log || goto :node1fail

::Change directory to access the test_results.txt file
cd ..
::Write to the file
echo Passed test: node 1 successfully connected to other nodes >> test_results.txt
::Change back
cd logs


::Check if node 2 got connected to node 1 and 3
findstr /c:"Heartbeat request from 1" paxos_2.log || goto :node2fail
findstr /c:"Heartbeat request from 3" paxos_2.log || goto :node2fail
cd ..
echo Passed test: node 2 successfully connected to other nodes >> test_results.txt
cd logs

::Check if node 3 got connected to node 1 and 2
findstr /c:"Heartbeat request from 1" paxos_3.log || goto :node3fail
findstr /c:"Heartbeat request from 2" paxos_3.log || goto :node3fail
cd ..
echo Passed test: node 3 successfully connected to other nodes >> test_results.txt
cd logs

::Check if nodes are able to elect a leader
findstr /c:"Newly elected leader" paxos_1.log || goto :leaderfail
findstr /c:"Newly elected leader" paxos_2.log || goto :leaderfail
findstr /c:"Newly elected leader" paxos_3.log || goto :leaderfail

cd ..
echo Passed test: the nodes can elect a leader >> test_results.txt
cd logs

::findstr /c:"did not get heartbeat from leader" || goto :
::findstr /c:""
::IF ()



::If all tests pass we are done
cd ..
goto :eof

::Fail cases for tests
:node1fail
cd ..
echo Failed test: node 1 was not properly connected >> test_results.txt
echo Ending test run prematurely, please correct the error >> test_results.txt
goto :eof

:node2fail
cd ..
echo Failed test: node 2 was not properly connected >> test_results.txt
echo Ending test run prematurely, please correct the error >> test_results.txt
goto :eof

:node3fail
cd ..
echo Failed test: node 3 was not properly connected >> test_results.txt
echo Ending test run prematurely, please correct the error >> test_results.txt
goto :eof

:leaderfail
cd ..
echo Failed test: nodes unable to elect a leader >> test_results.txt
echo Ending test run prematurely, please correct the error >> test_results.txt
goto :eof