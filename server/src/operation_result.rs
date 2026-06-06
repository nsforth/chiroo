pub trait OperationResult {
    fn is_success(&self) -> bool;

    fn is_failed(&self) -> bool {
        !self.is_success()
    }

    fn status(&self) -> &'static str {
        if self.is_success() {
            "success"
        } else {
            "failed"
        }
    }
}
