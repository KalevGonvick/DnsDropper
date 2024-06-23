use crate::exchange::Exchange;

pub(crate) trait PacketHandler {
    fn exec(&self, exchange: &mut Exchange);
}