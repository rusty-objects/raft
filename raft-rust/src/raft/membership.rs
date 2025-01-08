use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

/// A primary key for the node.  Use an opaque id rather than an address as the
/// id because addresses aren't unique.  They can be reused over time, and if a
/// host wipes its drives and comes back fresh, it might have the same address
/// as previously but is otherwise a different entity.  Having a random 64 bit
/// number be used as the id ensures that they host can come back with a new id
/// and act like a fresh member.
///
/// The default id, NodeId(0), indicates a dummy member whose the leader for
/// the root entry of the log (position 0).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, Debug, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// address for communicating with a node.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub String);
impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The cluster `Membership`, which can either be stable or in the process of transitioning
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Membership {

    /// Stable membership.  Decisions must be made in a majority of the set.
    Stable(HashMap<NodeId, Address>),

    /// Transitioning membership.  Decisions must be made in a majority of old and a majority of new
    Transitioning{ old: HashMap<NodeId, Address>, new: HashMap<NodeId, Address> },

}

impl Membership {

    // format: id@endpoint,id@endpoint,id@endpoint
    pub fn parse(input: String) -> Self {
        let members = input.split(",")
            .map(|member_string|  {
                let mut pieces = member_string.trim().split("@");
                let piece0 = pieces.next().unwrap();
                let id = NodeId(piece0.trim().parse().unwrap());
                let piece1 = pieces.next().unwrap();
                let addr = Address(piece1.trim().to_string());
                (id, addr)
            }).collect::<HashMap<NodeId, Address>>();
        Membership::Stable(members)
    }

    pub fn is_majority(&self, pool: &HashSet<NodeId>) -> bool {

        // an inner function!  nuts, right?
        // return true if the intersection of a and b is larger than half of b
        fn is_majority_(a: &HashSet<NodeId>, b: &HashMap<NodeId, Address>) -> bool {
            let mut count = 0;
            for i in a {
                if b.contains_key(i) {
                    count += 1;
                }
            }
            count > b.len()/2
        }

        match self {
            Membership::Stable(members) => is_majority_(pool, members),
            Membership::Transitioning{old, new} => is_majority_(pool, old) && is_majority_(pool, new),
        }
    }

    pub fn as_set(&self) -> HashSet<NodeId> {
        match self {
            Membership::Stable(map1) => map1.keys().map(|node_id| *node_id).collect(),
            Membership::Transitioning{old: map1, new: map2} => {
                let mut set = HashSet::new();
                map1.keys().for_each(|n| { set.insert(n.clone()); });
                map2.keys().for_each(|n| { set.insert(n.clone()); });
                set
            }
        }
    }

    pub fn address_of(&self, id: NodeId) -> Option<&Address> {
        match self {
            Membership::Stable(map1) => map1.get(&id),
            Membership::Transitioning{old: map1, new: map2} => {
                if map1.contains_key(&id) {
                    map1.get(&id)
                } else {
                    map2.get(&id)
                }
            },
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn test_majority_stable() {

        let a1 = Address("foo:1".to_string());
        let a2 = Address("foo:2".to_string());
        let a3 = Address("foo:3".to_string());
        let mut map = HashMap::new();
        map.insert(NodeId(1), a1);
        map.insert(NodeId(2), a2);
        map.insert(NodeId(3), a3);
        let membership = Membership::Stable(map);

        let mut set = HashSet::new();
        set.insert(NodeId(1));
        assert!(!membership.is_majority(&set));

        set.insert(NodeId(2));
        assert!(membership.is_majority(&set));

        // now make sure it's not just raw count that matters
        set = HashSet::new();
        set.insert(NodeId(111));
        set.insert(NodeId(222));
        assert!(!membership.is_majority(&set));
    }

    #[test]
    pub fn test_majority_transitioning() {

        let a1 = Address("foo:1".to_string());
        let a2 = Address("foo:2".to_string());
        let a3 = Address("foo:3".to_string());
        let mut map1 = HashMap::new();
        map1.insert(NodeId(1), a1);
        map1.insert(NodeId(2), a2);
        map1.insert(NodeId(3), a3);

        let a4 = Address("foo:4".to_string());
        let a5 = Address("foo:5".to_string());
        let a6 = Address("foo:6".to_string());
        let mut map2 = HashMap::new();
        map2.insert(NodeId(4), a4);
        map2.insert(NodeId(5), a5);
        map2.insert(NodeId(6), a6);

        let membership = Membership::Transitioning {old: map1, new: map2};

        let mut set = HashSet::new();
        set.insert(NodeId(1));
        set.insert(NodeId(2));
        set.insert(NodeId(4));
        assert!(!membership.is_majority(&set));

        set.insert(NodeId(5));
        assert!(membership.is_majority(&set));
    }

    #[test]
    fn parse() {
        let mut map = HashMap::new();
        map.insert(NodeId(1), Address("abc:123".to_string()));
        map.insert(NodeId(2), Address("xyz:789".to_string()));
        map.insert(NodeId(3), Address("mmm:555".to_string()));
        let membership = Membership::Stable(map);
        let membership1 = Membership::parse("1@abc:123,2@xyz:789,3@mmm:555".to_string());
        let membership2 = Membership::parse("1@abc:123, 2@xyz:789, 3@mmm:555".to_string());
        assert_eq!(membership, membership1);
        assert_eq!(membership, membership2);

    }
}
