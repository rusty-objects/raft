#!/bin/bash

# this script needs work...
# on my mac I had to run the following to allow listening on 127.0.0.X for X>1:
# for i in `seq 2 30`; do echo $i; sudo ifconfig lo0 alias 127.0.0.$i; done;

echo "starting servers...";

./target/debug/raft-rust server 1 "1@127.0.0.1:10001, 2@127.0.0.1:10002, 3@127.0.0.1:10003" 1 0 2 10.0.1.2:12345 &
./target/debug/raft-rust server 2 "1@127.0.0.1:10001, 2@127.0.0.1:10002, 3@127.0.0.1:10003" 2 1 2 10.0.1.2:12345 &
./target/debug/raft-rust server 3 "1@127.0.0.1:10001, 2@127.0.0.1:10002, 3@127.0.0.1:10003" 3 2 2 10.0.1.2:12345 &
#./target/debug/raft-rust server 127.0.0.2:10001 "127.0.0.2:10001, 127.0.0.3:10001, 127.0.0.4:10001" 1 0 2 10.0.1.2:12345 &
#./target/debug/raft-rust server 127.0.0.3:10001 "127.0.0.2:10001, 127.0.0.3:10001, 127.0.0.4:10001" 2 1 2 10.0.1.2:12345 &
#./target/debug/raft-rust server 127.0.0.4:10001 "127.0.0.2:10001, 127.0.0.3:10001, 127.0.0.4:10001" 3 2 2 10.0.1.2:12345 &

echo "sending color commands...";

send_color() {
    MSG="CMD [$((RANDOM%255)),$((RANDOM%255)),$((RANDOM%255))]";
    for ip in "127.0.0.2" "127.0.0.3" "127.0.0.4"; do 
        echo "--- $MSG | nc $ip 10001 ---";
        echo $MSG | nc $ip 10001;
        echo "";
    done

    sleep 2;
}

firewall() {
    echo "firewall $1"
}

unfirewall() {
    echo "unfirewall $1"
}

for i in {1..5}; do
    for ip in "127.0.0.2" "127.0.0.3" "127.0.0.4"; do 
        # send color messages once in a while to everyone
        for i in {1..15}; do 
            send_color;
        done

        # firewall off one dude
        firewall $ip;

        # send color messages once in a while with the dude firewalled
        for i in {1..15}; do 
            send_color;
        done

        # bring firewalled guy back
        unfirewall $ip;
    done
done

killall raft-rust
