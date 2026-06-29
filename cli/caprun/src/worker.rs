/// caprun-worker — confined worker binary (Phase 3 demo)
///
/// Phase 3 Wave 0: stub binary. Wave 2 Plan 05 implements the full worker:
///   1. Connect to broker UDS abstract socket (env: BROKER_SOCK_ABSTRACT)
///   2. Send BrokerRequest::CreateSession
///   3. Send BrokerRequest::RequestFd { path }
///   4. Receive fd via SCM_RIGHTS (adapter-fs::recv_fd)
///   5. Read file via fd, send BrokerRequest::ReportRead { bytes_read }
///   6. Exit cleanly

fn main() {}
