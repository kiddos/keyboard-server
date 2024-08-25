
#include <gflags/gflags.h>
#include <glog/logging.h>
#include <grpcpp/grpcpp.h>

#include <csignal>

#include "apm_calldata.h"
#include "keyboard.grpc.pb.h"

using kb::KeyboardService;
using grpc::CompletionQueue;
using grpc::Server;
using grpc::ServerBuilder;
using grpc::ServerCompletionQueue;

DEFINE_int32(port, 50051, "server address port");
DEFINE_int32(interval, 120, "time interval to average key APM (in seconds)");

std::unique_ptr<ServerCompletionQueue> cq;
std::unique_ptr<Server> server;

void Cleanup() {
  LOG(INFO) << "shutdown server...";
  if (server) {
    server->Shutdown();
  }
  if (cq) {
    cq->Shutdown();
  }
}

void SignalInterrupt(int) {
  LOG(INFO) << "interrupt detected...";

  Cleanup();
  exit(0);
}

void MainLoop(KeyboardService::AsyncService& service) {
  new APMCallData(&service, cq.get());
  void* tag = nullptr;  // uniquely identifies a request.
  bool ok = false;

  KeyboardListener listener(FLAGS_interval);
  while (true) {
    listener.ReadAPM();

    // Block waiting to read the next event from the completion queue. The
    // event is uniquely identified by its tag, which in this case is the
    // memory address of a CallData instance.
    // The return value of Next should always be checked. This return value
    // tells us whether there is any kind of event or cq_ is shutting down.
    using namespace std::literals;
    auto deadline = std::chrono::system_clock::now() + 66ms;
    auto next_status = cq->AsyncNext(&tag, &ok, deadline);
    if (next_status == CompletionQueue::NextStatus::GOT_EVENT && tag && ok) {
      static_cast<APMCallData*>(tag)->Proceed();
    }
  }
}

void RunServer() {
  std::string server_address = "localhost:" + std::to_string(FLAGS_port);

  ServerBuilder builder;
  // Listen on the given address without any authentication mechanism.
  builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
  // Register "service_" as the instance through which we'll communicate with
  // clients. In this case it corresponds to an *asynchronous* service.
  KeyboardService::AsyncService service;
  builder.RegisterService(&service);
  // Get hold of the completion queue used for the asynchronous communication
  // with the gRPC runtime.
  cq = builder.AddCompletionQueue();
  // Finally assemble the server.
  server = builder.BuildAndStart();
  LOG(INFO) << "Server listening on " << server_address;

  MainLoop(service);
}

int main(int argc, char* argv[]) {
  google::InitGoogleLogging(argv[0]);
  FLAGS_logtostdout = 1;

  gflags::ParseCommandLineFlags(&argc, &argv, true);

  signal(SIGINT, SignalInterrupt);
  RunServer();

  Cleanup();
  return 0;
}
