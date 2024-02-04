#[derive(Hash, Eq, PartialEq, Debug)]
pub(crate) struct Filter {
    pub address: String,
    pub domain: String
}

impl Filter {
    fn should_filter(&self, address: String, domain: String) -> bool {
        return address.eq(&self.address) && domain.eq(&self.domain);
    }
}