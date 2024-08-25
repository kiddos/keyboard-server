#include <gflags/gflags.h>
#include <grpcpp/grpcpp.h>

#include <iostream>
#include <memory>

#include "keyboard.grpc.pb.h"

using kb::APMRequest;
using kb::APMResponse;
using kb::KeyboardService;
using grpc::Channel;
using grpc::ClientContext;
using grpc::Status;

DEFINE_int32(port, 50051, "server address port");

class APMClient {
 public:
  APMClient(std::shared_ptr<Channel> channel)
      : stub_(KeyboardService::NewStub(channel)) {}

  double GetAPM() {
    APMRequest request;
    APMResponse response;

    // Context for the client. It could be used to convey extra information to
    // the server and/or tweak certain RPC behaviors.
    ClientContext context;
    Status status = stub_->GetAPM(&context, request, &response);

    if (status.ok()) {
      return response.apm();
    } else {
      return 0;
    }
  }

 private:
  std::unique_ptr<KeyboardService::Stub> stub_;
};

int main(int argc, char* argv[]) {
  gflags::ParseCommandLineFlags(&argc, &argv, true);

  const std::string address = "localhost:" + std::to_string(FLAGS_port);
  APMClient greeter(
      grpc::CreateChannel(address, grpc::InsecureChannelCredentials()));
  std::cout << std::fixed << std::setprecision(3) << greeter.GetAPM() << std::endl;
  return 0;
}
