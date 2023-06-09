use core::fmt;
use serde::{Serialize, Deserialize};
use crate::{util::timestamp, err::{TResult, TellErr, LibErr}};

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id {
    name: String,
    sign: u128
}

impl Id {
    pub fn new(name: String) -> TResult<Id> {
        Self::verify_name(&name)?;
        Ok(Id {
            name, sign: timestamp()
        })
    }

    fn verify_name(name: &String) -> TResult {
        if name.len() > 10 || name.len() < 3 {
            Err(TellErr::Lib(LibErr::InvalidName(name.clone())))
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}{}/", self.name, self.sign)
    }
}
