/// The interface that all Branch Predictors must implement.
pub trait BranchPredictor {
    /// Returns (Predicted Taken?, Optional Target Address)
    fn predict_branch(&self, pc: u64) -> (bool, Option<u64>);

    /// Updates the predictor tables based on actual execution results
    fn update_branch(&mut self, pc: u64, taken: bool, target: Option<u64>);

    /// Look up a target in the Branch Target Buffer
    fn predict_btb(&self, pc: u64) -> Option<u64>;

    /// Handle function calls (push to RAS, update BTB)
    fn on_call(&mut self, pc: u64, ret_addr: u64, target: u64);

    /// Predict return address from RAS
    fn predict_return(&self) -> Option<u64>;

    /// Handle function returns (pop from RAS)
    fn on_return(&mut self);
}
