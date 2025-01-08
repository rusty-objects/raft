## TODOs
### High Level
* Move these to issue tracker
* This performs relatively well on my mac, but like crap on my pi.  Connections are stalled/refused constantly and this leads to clients being unable to communicate to servers, server messages delaying to each other by 20 seconds or more, and frequent reelection.  This is a partial failure, as some messages are indeed getting through.
* Log Compaction/Snapshotting
* Membership Changes
* Durable Log
* Membership discovery

### Specific
* Make sure the entries being inserted into the log are actually for the correct index
* Ignore re-transmitting to servers if their match index is greater than their NACK (implies delayed NACK)
* Have leaders who haven't received an ACK within election timeout to turn into followers
* ACK the client properly (only after the value is committed, and send back the result of the state machine apply, not just the log apply)
