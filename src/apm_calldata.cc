#include "apm_calldata.h"

#include <glog/logging.h>

#include <fstream>

static double current_apm = 0;

std::vector<std::string> split(const std::string& s, char d) {
  std::stringstream ss;
  ss << s;
  std::vector<std::string> output;
  std::string p;
  while (std::getline(ss, p, d)) {
    output.push_back(p);
  }
  return output;
}

bool starts_with(const std::string& s, const std::string& p) {
  return s.substr(0, p.length()) == p;
}

KeyboardListener::KeyboardListener(int interval) : interval_(interval * 1000) {
  FindKeyboardDevice();

  // X11 display
  LOG(INFO) << "get X11 display handle...";
  display_ = XOpenDisplay(nullptr);
}

KeyboardListener::~KeyboardListener() {
  if (display_) {
    XCloseDisplay(display_);
  }
}

void KeyboardListener::FindKeyboardDevice() {
  std::ifstream device_file("/proc/bus/input/devices", std::ifstream::in);
  if (device_file.is_open()) {
    std::string line;
    std::string handler;
    while (std::getline(device_file, line)) {
      if (line.empty()) {
        continue;
      }

      const std::string header = line.substr(0, 2);
      const std::string values = line.substr(3);
      if (header == "H:") {
        // handlers
        auto idx = values.find("=");
        if (idx != std::string::npos) {
          const std::string handlers_str = values.substr(idx);
          const std::vector<std::string> handlers = split(handlers_str, ' ');
          for (const std::string& h : handlers) {
            if (starts_with(h, "event")) {
              handler = h;
            }
          }
        }
      } else if (header == "B:") {
        auto idx = values.find("=");
        if (idx != std::string::npos) {
          const std::string key = values.substr(0, idx);
          const std::string value = values.substr(idx + 1);
          if (key == "EV" && value == "120013") {
            if (!handler.empty()) {
              dev_ = "/dev/input/" + handler;
              LOG(INFO) << "keyboard device: " << dev_;
              return;
            }
          }
        }
      }
    }
  }
}

u64 get_system_millis() {
  return std::chrono::duration_cast<std::chrono::milliseconds>(
             std::chrono::system_clock::now().time_since_epoch())
      .count();
}

void KeyboardListener::ReadAPM() {
  if (dev_.empty()) {
    return;
  }

  // int fd = open(dev_.c_str(), O_RDONLY | O_NONBLOCK);
  // if (fd < 0) {
  //   return;
  // }
  //
  // struct timeval cur;
  // gettimeofday(&cur, NULL);
  // while (!d_.empty() && cur.tv_sec - d_.front().time.tv_sec > interval_) {
  //   d_.pop_front();
  // }
  //
  // struct input_event event;
  // // std::cout << "size=" << sizeof(event) << std::endl;
  // struct pollfd pollfd = {
  //     .fd = fd,
  //     .events = POLLIN,
  //     .revents = 0,
  // };
  // int result = poll(&pollfd, 1, 100);
  // if (result < 0) {
  //   return;
  // }
  //
  // if (pollfd.revents & POLLIN) {
  //   while (read(fd, &event, sizeof(event)) == sizeof(event)) {
  //     if (event.type == EV_KEY) {
  //       d_.push_back(event);
  //     }
  //   }
  // }

  if (!display_) {
    return;
  }

  Window current;
  int revert;
  XGetInputFocus(display_, &current, &revert);
  XSelectInput(display_, current, KeyPressMask);

  while (XPending(display_)) {
    XEvent event;
    XNextEvent(display_, &event);
    switch (event.type) {
      case KeyPress:
        u32 keycode = event.xkey.keycode;
        u64 t = get_system_millis();
        d_.push_back({keycode, t});
        break;
    }
  }

  u64 current_t = get_system_millis();
  while (!d_.empty() && current_t - d_.front().t >= interval_) {
    d_.pop_front();
  }
  if (d_.empty()) {
    current_apm = 0;
  } else {
    double time_passed = current_t - d_.front().t + 1e-6;
    current_apm = (double)d_.size() / time_passed * 60 * 1000;
  }
}

APMCallData::APMCallData(KeyboardService::AsyncService* service,
                         ServerCompletionQueue* cq)
    : service_(service), cq_(cq), responder_(&ctx_), status_(INIT) {
  Proceed();
}

void APMCallData::Proceed() {
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
