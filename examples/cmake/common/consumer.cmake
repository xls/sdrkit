# Builds stack_count with sdrkit::sdrkit and registers it as a test. The three consumer projects include this file
# after they have defined the target sdrkit::sdrkit.
add_executable(stack_count "${CMAKE_CURRENT_LIST_DIR}/stack_count.c")
target_link_libraries(stack_count PRIVATE sdrkit::sdrkit)
set_target_properties(stack_count PROPERTIES C_STANDARD 99 C_STANDARD_REQUIRED ON)

# On Windows a program finds sdrkit.dll in its own folder: copy it there after the link.
get_target_property(sdrkit_linked sdrkit::sdrkit ALIASED_TARGET)
if(WIN32 AND sdrkit_linked STREQUAL "sdrkit::shared")
    add_custom_command(TARGET stack_count POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different $<TARGET_RUNTIME_DLLS:stack_count>
            $<TARGET_FILE_DIR:stack_count>
        COMMAND_EXPAND_LISTS)
endif()

message(STATUS "stack_count links ${sdrkit_linked}")
enable_testing()
add_test(NAME stack_count COMMAND stack_count)
set_tests_properties(stack_count PROPERTIES PASS_REGULAR_EXPRESSION "sdrkit [0-9.]+: [1-9][0-9]* stacks")
