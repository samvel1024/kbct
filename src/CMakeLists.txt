set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} -Wall -g")

add_subdirectory(poll)

add_executable(laykeymap main.cpp keyboard.c KeyboardGrabManager.h KeymapConfig.h)
target_link_libraries(laykeymap PRIVATE nlohmann_json::nlohmann_json poll evdev)

