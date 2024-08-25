include(FetchContent)

# X11
find_package(X11 REQUIRED)

# glog
FetchContent_Declare(
  glog
  GIT_REPOSITORY https://github.com/google/glog
  GIT_TAG        v0.7.1
  EXCLUDE_FROM_ALL
)
FetchContent_MakeAvailable(glog)

# gflags
FetchContent_Declare(
  gflags
  GIT_REPOSITORY https://github.com/gflags/gflags.git
  GIT_TAG        v2.2.2
  EXCLUDE_FROM_ALL
)
FetchContent_MakeAvailable(gflags)

# grpc
FetchContent_Declare(
  gRPC
  GIT_REPOSITORY https://github.com/grpc/grpc
  GIT_TAG        v1.65.5
  EXCLUDE_FROM_ALL
)
set(FETCHCONTENT_QUIET OFF)
set(ABSL_ENABLE_INSTALL ON)
FetchContent_MakeAvailable(gRPC)

# Since FetchContent uses add_subdirectory under the hood, we can use
# the grpc targets directly from this build.
set(_PROTOBUF_LIBPROTOBUF libprotobuf)
set(_REFLECTION grpc++_reflection)
set(_PROTOBUF_PROTOC $<TARGET_FILE:protoc>)
set(_GRPC_GRPCPP grpc++)
if(CMAKE_CROSSCOMPILING)
  find_program(_GRPC_CPP_PLUGIN_EXECUTABLE grpc_cpp_plugin)
else()
  set(_GRPC_CPP_PLUGIN_EXECUTABLE $<TARGET_FILE:grpc_cpp_plugin>)
endif()

get_filename_component(kb_proto "${CMAKE_SOURCE_DIR}/proto/keyboard.proto" ABSOLUTE)
get_filename_component(kb_proto_path "${kb_proto}" PATH)

set(kb_proto_srcs "${CMAKE_CURRENT_BINARY_DIR}/keyboard.pb.cc")
set(kb_proto_hdrs "${CMAKE_CURRENT_BINARY_DIR}/keyboard.pb.h")
set(kb_grpc_srcs "${CMAKE_CURRENT_BINARY_DIR}/keyboard.grpc.pb.cc")
set(kb_grpc_hdrs "${CMAKE_CURRENT_BINARY_DIR}/keyboard.grpc.pb.h")
include_directories("${CMAKE_CURRENT_BINARY_DIR}")
add_custom_command(
  OUTPUT "${kb_proto_srcs}" "${kb_proto_hdrs}" "${kb_grpc_srcs}" "${kb_grpc_hdrs}"
  COMMAND ${_PROTOBUF_PROTOC}
  ARGS --grpc_out "${CMAKE_CURRENT_BINARY_DIR}"
    --cpp_out "${CMAKE_CURRENT_BINARY_DIR}"
    -I "${kb_proto_path}"
    --plugin=protoc-gen-grpc="${_GRPC_CPP_PLUGIN_EXECUTABLE}"
    "${kb_proto}"
  DEPENDS "${kb_proto}")


add_library(kb_grpc_proto STATIC
  ${kb_grpc_srcs}
  ${kb_grpc_hdrs}
  ${kb_proto_srcs}
  ${kb_proto_hdrs})
target_link_libraries(kb_grpc_proto
  ${_REFLECTION}
  ${_GRPC_GRPCPP}
  ${_PROTOBUF_LIBPROTOBUF})

add_executable(keyboard_server keyboard_server.cc apm_calldata.cc)
target_link_libraries(keyboard_server
  kb_grpc_proto
  ${_REFLECTION}
  ${_GRPC_GRPCPP}
  ${_PROTOBUF_LIBPROTOBUF}
  ${X11_LIBRARIES}
  glog::glog
  gflags::gflags
)

add_executable(apm_client apm_client.cc)
target_link_libraries(apm_client
  kb_grpc_proto
  ${_REFLECTION}
  ${_GRPC_GRPCPP}
  ${_PROTOBUF_LIBPROTOBUF}
  gflags::gflags
)
