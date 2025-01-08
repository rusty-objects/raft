Commit descriptions from before 2019.

##### Mon Dec 31 19:17:46 2018 +0000 
 **fix test** 
 
 
##### Mon Dec 31 17:11:54 2018 +0000 
 **Update wire format docs to use node_id** 
 
 
##### Mon Dec 31 16:45:50 2018 +0000 
 **fix launcher script** 
 
 
##### Mon Dec 31 16:40:53 2018 +0000 
 **Merge remote-tracking branch 'origin/rust'** 
 
 
##### Mon Dec 31 16:34:34 2018 +0000 
 **Make node id the primary key for members** 
 
 Instead of using endpoint address as the primary key, use an opaque token.  This allows recycling nodes more easily because we don't have to worry about future nodes sharing the ip addresses of previous nodes, as long as they generate a unique node id on each occurrence, they'll be considered unique.

##### Mon Dec 31 06:37:31 2018 +0000 
 **typo in readme** 
 
 
##### Mon Dec 31 06:22:49 2018 +0000 
 **Update readme** 
 
 
##### Mon Dec 31 05:41:23 2018 +0000 
 **Bug fix: latest position in append_if** 
 
 The latest position was being calculated wrong for an append_if that redacted entries.

##### Mon Dec 31 05:34:29 2018 +0000 
 **Make heartbeats stateless** 
 
 I don't need to write to the log on every heartbeat.  The paper calls for simply propagating an empty log.  This allows us to retain our leadership without filling up the logs with needless entries, and is overall goodness.  It also puts less pressure on building log compaction.

##### Mon Dec 31 05:34:11 2018 +0000 
 **Fix formatting** 
 
 
##### Sun Dec 30 17:31:26 2018 +0000 
 **Scaffolding for java app** 
 
 
##### Sun Dec 30 16:51:20 2018 +0000 
 **Added examples for wire protocol** 
 
 
##### Sun Dec 30 10:56:45 2018 +0000 
 **Add perf TODO to README** 
 
 
##### Sun Dec 30 10:55:11 2018 +0000 
 **Updated README based on rust impl** 
 
 
##### Sun Dec 30 10:39:51 2018 +0000 
 **Initial rust implementation** 
 
 This implementation functions over the real network.  It allows for clients to send in commands, and for the commands to be replicated and applied.  In the in-memory version (see main.rs) there is a mechanism for testing isolated clients.  In the real-world version (see launch.sh), the firewall commands do not yet exist.

This peforms well on my mac, but doesn't work on the raspberry pi.  It generally functions, but the networking appears to be terrible, and messages are being delayed like crazy.  In one run, the leader was seeing ACKs for records that the follower sent 20 seconds prior.  Consequently the leader is retrasmitting a whole bunch.  The entire thing could benefit from connection pooling I suspect, but that shouldn't be required for this low message rate.  Slowing things down didn't help: client connections timeout and re-election happens often.

##### Sun Dec 30 09:57:05 2018 +0000 
 **Simplifying pi server** 
 
 Just provide a simple API for setting pixels.  Let the applications themselves decide how to arrange pixels in a pleasing way, rather than baking that into more complex APIs on the sense_hat.

##### Fri Dec 28 18:35:03 2018 +0000 
 **Add ability to set a stripe** 
 
 The new plan is to have a small set of members (<=7).  To decide who is the leader, we'll set a stripe across the top of the display of length equal to the leader's index.  Each member will present their color as a pixel, and we'll be able to tell which of those members is acting as leader.

##### Mon Dec 10 21:12:22 2018 +0000 
 **Work in progress** 
 
 This is a partial implementation of raft in rust.  Far from done at this point, thsi is just a set of abstractions for organizing a state machine.

##### Mon Dec 10 21:02:45 2018 +0000 
 **First commit** 
 
 Contains boilerplate repo files and a python udp server for interacting with raspberry pi sense hat LEDs.
