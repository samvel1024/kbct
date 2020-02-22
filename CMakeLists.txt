cmake_minimum_required(VERSION 3.10)
project(laykeymap)
enable_language(CXX)
set(CMAKE_CXX_STANDARD 17)

set(JSON_BuildTests OFF CACHE INTERNAL "")
add_subdirectory(json)

add_compile_options("-g")

add_executable(simkey simkey.cpp keyboard.c)
target_link_libraries(simkey PRIVATE nlohmann_json::nlohmann_json)
