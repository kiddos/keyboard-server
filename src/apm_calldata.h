#ifndef APM_CALLDATA_H
#define APM_CALLDATA_H

// #include <X11/Xlib.h>
// #include <X11/Xutil.h>
// #include <X11/extensions/XInput.h>
// #undef Status

#include <glog/logging.h>

#include "call_status.h"
#include "keyboard.grpc.pb.h"

using grpc::ServerAsyncResponseWriter;
using grpc::ServerCompletionQueue;
using grpc::ServerContext;
using kb::APMRequest;
using kb::APMResponse;
using kb::KeyboardService;
using u32 = unsigned int;
using u64 = unsigned long long;

struct KeyEvent {
  u32 keycode;
  u64 t;
};

class KeyboardListener {
 public:
  KeyboardListener(int interval);
  ~KeyboardListener();

  void FindKeyboardDevice();
  void PrepareFileDescriptor();
  void ReadAPM();

 private:
  // Display* display_;
  int fd_;
  u64 interval_;
  std::string dev_;
  std::deque<KeyEvent> d_;
};

class APMCallData {
 public:
  APMCallData(KeyboardService::AsyncService* service,
              ServerCompletionQueue* cq);

  void Proceed();

 private:
  KeyboardService::AsyncService* service_;
  ServerCompletionQueue* cq_;
  ServerContext ctx_;

  APMRequest request_;
  APMResponse response_;

  ServerAsyncResponseWriter<APMResponse> responder_;

  CallStatus status_;  // The current serving state.
};

#endif /* end of include guard: APM_CALLDATA_H */
