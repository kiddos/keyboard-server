#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/XInput.h>
#undef Status

#include <gflags/gflags.h>
#include <glog/logging.h>
#include <grpcpp/grpcpp.h>

#include <csignal>

#include "keyboard.grpc.pb.h"

using kb::APMRequest;
using kb::APMResponse;
using kb::KeyboardService;
using grpc::CompletionQueue;
using grpc::Server;
using grpc::ServerAsyncResponseWriter;
using grpc::ServerBuilder;
using grpc::ServerCompletionQueue;
using grpc::ServerContext;
using u32 = unsigned int;
using u64 = unsigned long long;

DEFINE_int32(port, 50051, "server address port");

constexpr u64 MAX_TIMEPASS = 60 * 1000;

std::unique_ptr<ServerCompletionQueue> cq;
std::unique_ptr<Server> server;
Display* display = nullptr;
static double current_apm = 0;

void cleanup() {
  LOG(INFO) << "shutdown server...";
  if (server) {
    server->Shutdown();
  }
  if (cq) {
    cq->Shutdown();
  }

  if (display) {
    XCloseDisplay(display);
  }
}

void signal_interrupt(int) {
  cleanup();
  exit(0);
}

class APMCallData {
 public:
  APMCallData(KeyboardService::AsyncService* service, ServerCompletionQueue* cq)
      : service_(service), cq_(cq), responder_(&ctx_), status_(INIT) {
    // Invoke the serving logic right away.
    Proceed();
  }

  void Proceed() {
    if (status_ == INIT) {
      // Make this instance progress to the PROCESS state.
      status_ = PROCESS;

      service_->RequestGetAPM(&ctx_, &request_, &responder_, cq_, cq_, this);
    } else if (status_ == PROCESS) {
      // Spawn a new CallData instance to serve new clients while we process
      // the one for this CallData. The instance will deallocate itself as
      // part of its FINISH state.
      new APMCallData(service_, cq_);

      response_.set_apm(current_apm);

      // And we are done! Let the gRPC runtime know we've finished, using the
      // memory address of this instance as the uniquely identifying tag for
      // the event.
      status_ = FINISH;
      responder_.Finish(response_, grpc::Status::OK, this);
    } else {
      GPR_ASSERT(status_ == FINISH);
      // Once in the FINISH state, deallocate ourselves (CallData).
      delete this;
    }
  }

 private:
  KeyboardService::AsyncService* service_;
  ServerCompletionQueue* cq_;
  ServerContext ctx_;

  APMRequest request_;
  APMResponse response_;

  ServerAsyncResponseWriter<APMResponse> responder_;

  enum CallStatus { INIT, PROCESS, FINISH };
  CallStatus status_;  // The current serving state.
};

u64 get_system_millis() {
  return std::chrono::duration_cast<std::chrono::milliseconds>(
             std::chrono::system_clock::now().time_since_epoch())
      .count();
}

void main_loop(KeyboardService::AsyncService& service) {
  // X11 display
  LOG(INFO) << "get X11 display handle...";
  display = XOpenDisplay(NULL);

  new APMCallData(&service, cq.get());
  void* tag = nullptr;  // uniquely identifies a request.
  bool ok = false;
  std::deque<std::pair<u32, u64>> keys;
  while (true) {
    Window current;
    int revert;
    XGetInputFocus(display, &current, &revert);
    XSelectInput(display, current, KeyPressMask);

    while (XPending(display)) {
      XEvent event;
      XNextEvent(display, &event);
      switch (event.type) {
        case KeyPress:
          u32 keycode = event.xkey.keycode;
          u64 t = get_system_millis();
          keys.push_back({keycode, t});
          break;
      }
    }

    u64 current_t = get_system_millis();
    while (!keys.empty() && current_t - keys.front().second >= MAX_TIMEPASS) {
      keys.pop_front();
    }
    if (keys.empty()) {
      current_apm = 0;
    } else {
      u64 time_passed = current_t - keys.front().second;
      current_apm = (double)keys.size() / time_passed * 60 * 1000;
    }

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

void run_server() {
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

  main_loop(service);
}

int main(int argc, char* argv[]) {
  google::InitGoogleLogging(argv[0]);
  FLAGS_logtostdout = 1;

  gflags::ParseCommandLineFlags(&argc, &argv, true);

  signal(SIGINT, signal_interrupt);
  run_server();

  cleanup();
  return 0;
}
