use crate::composition::dependency_graph::DependencyGraph;

pub(crate) struct DiagnosticsFacade {
    dependencies: DependencyGraph,
}

impl DiagnosticsFacade {
    pub(crate) fn new(dependencies: DependencyGraph) -> Self {
        Self { dependencies }
    }

    pub(crate) fn warn(&self, message: &str) {
        self.dependencies.diagnostics.warn(message);
    }
}
