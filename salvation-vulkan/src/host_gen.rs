use salvation_core::compiler::ast::types::{Item, Param, Program, ShaderStage};

#[derive(Debug, Default)]
pub struct ShaderInfo {
    pub vert_fn: Option<String>,
    pub frag_fn: Option<String>,
    pub kernel_fn: Option<String>,
    pub kernel_params: Vec<Param>,
}

pub fn analyze(program: &Program) -> ShaderInfo {
    let mut info = ShaderInfo::default();

    for item in program {
        if let Item::FnDecl { stage, name, params, .. } = item {
            match stage {
                Some(ShaderStage::Vertex) => {
                    info.vert_fn = Some(name.clone());
                }
                Some(ShaderStage::Fragment) => {
                    info.frag_fn = Some(name.clone());
                }
                Some(ShaderStage::Kernel) => {
                    info.kernel_fn = Some(name.clone());
                    info.kernel_params = params.clone();
                }
                _ => {}
            }
        }
    }

    info
}

pub fn gen_main_cpp(info: &ShaderInfo) -> String {
    if info.kernel_fn.is_some() && info.vert_fn.is_none() {
        gen_vulkan_compute_main(info)
    } else if info.vert_fn.is_some() {
        gen_vulkan_graphics_main(info)
    } else {
        String::from("// No shader functions found\n")
    }
}

fn gen_vulkan_compute_main(info: &ShaderInfo) -> String {
    let kernel_name = info.kernel_fn.as_ref().map(|s| s.as_str()).unwrap_or("kernel");

    let mut c = String::new();

    c.push_str("#include <vulkan/vulkan.h>\n");
    c.push_str("#include <cstdio>\n");
    c.push_str("#include <cstdint>\n");
    c.push_str("#include <cstring>\n");
    c.push_str("#include <vector>\n");
    c.push_str("#include <fstream>\n");
    c.push_str("#include <stdexcept>\n");
    c.push_str("#include <cstdlib>\n");
    c.push_str("\n");
    c.push_str("static std::vector<uint32_t> load_spv(const char* path) {\n");
    c.push_str("    FILE* f = fopen(path, \"rb\");\n");
    c.push_str("    if (!f) { fprintf(stderr, \"Cannot open %s\\n\", path); exit(1); }\n");
    c.push_str("    fseek(f, 0, SEEK_END);\n");
    c.push_str("    size_t size = ftell(f);\n");
    c.push_str("    fseek(f, 0, SEEK_SET);\n");
    c.push_str("    std::vector<uint32_t> buf(size / 4);\n");
    c.push_str("    fread(buf.data(), 4, buf.size(), f);\n");
    c.push_str("    fclose(f);\n");
    c.push_str("    return buf;\n");
    c.push_str("}\n");
    c.push_str("\n");
    c.push_str("#define VK_CHECK(x) do { VkResult r = (x); if (r != VK_SUCCESS) { fprintf(stderr, \"Vulkan error %d at line %d\\n\", r, __LINE__); exit(1); } } while(0)\n");
    c.push_str("\n");
    c.push_str("int main() {\n");
    c.push_str("    // Create Vulkan instance\n");
    c.push_str("    VkApplicationInfo appInfo{};\n");
    c.push_str("    appInfo.sType = VK_STRUCTURE_TYPE_APPLICATION_INFO;\n");
    c.push_str("    appInfo.apiVersion = VK_API_VERSION_1_0;\n");
    c.push_str("\n");
    c.push_str("    VkInstanceCreateInfo instCI{};\n");
    c.push_str("    instCI.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;\n");
    c.push_str("    instCI.pApplicationInfo = &appInfo;\n");
    c.push_str("\n");
    c.push_str("    VkInstance instance;\n");
    c.push_str("    VK_CHECK(vkCreateInstance(&instCI, nullptr, &instance));\n");
    c.push_str("\n");
    c.push_str("    // Find physical device\n");
    c.push_str("    uint32_t devCount = 0;\n");
    c.push_str("    vkEnumeratePhysicalDevices(instance, &devCount, nullptr);\n");
    c.push_str("    std::vector<VkPhysicalDevice> physDevs(devCount);\n");
    c.push_str("    vkEnumeratePhysicalDevices(instance, &devCount, physDevs.data());\n");
    c.push_str("    VkPhysicalDevice physDev = physDevs[0];\n");
    c.push_str("\n");
    c.push_str("    // Find compute queue family\n");
    c.push_str("    uint32_t qfCount = 0;\n");
    c.push_str("    vkGetPhysicalDeviceQueueFamilyProperties(physDev, &qfCount, nullptr);\n");
    c.push_str("    std::vector<VkQueueFamilyProperties> qfProps(qfCount);\n");
    c.push_str("    vkGetPhysicalDeviceQueueFamilyProperties(physDev, &qfCount, qfProps.data());\n");
    c.push_str("\n");
    c.push_str("    uint32_t computeQF = UINT32_MAX;\n");
    c.push_str("    for (uint32_t i = 0; i < qfCount; i++) {\n");
    c.push_str("        if (qfProps[i].queueFlags & VK_QUEUE_COMPUTE_BIT) {\n");
    c.push_str("            computeQF = i;\n");
    c.push_str("            break;\n");
    c.push_str("        }\n");
    c.push_str("    }\n");
    c.push_str("    if (computeQF == UINT32_MAX) { fprintf(stderr, \"No compute queue\\n\"); exit(1); }\n");
    c.push_str("\n");
    c.push_str("    // Create logical device\n");
    c.push_str("    float qPriority = 1.0f;\n");
    c.push_str("    VkDeviceQueueCreateInfo qCI{};\n");
    c.push_str("    qCI.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;\n");
    c.push_str("    qCI.queueFamilyIndex = computeQF;\n");
    c.push_str("    qCI.queueCount = 1;\n");
    c.push_str("    qCI.pQueuePriorities = &qPriority;\n");
    c.push_str("\n");
    c.push_str("    VkDeviceCreateInfo devCI{};\n");
    c.push_str("    devCI.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO;\n");
    c.push_str("    devCI.queueCreateInfoCount = 1;\n");
    c.push_str("    devCI.pQueueCreateInfos = &qCI;\n");
    c.push_str("\n");
    c.push_str("    VkDevice device;\n");
    c.push_str("    VK_CHECK(vkCreateDevice(physDev, &devCI, nullptr, &device));\n");
    c.push_str("\n");
    c.push_str("    VkQueue queue;\n");
    c.push_str("    vkGetDeviceQueue(device, computeQF, 0, &queue);\n");
    c.push_str("\n");
    c.push_str("    // Load SPIR-V\n");
    c.push_str("    auto spv = load_spv(\"shader.comp.spv\");\n");
    c.push_str("    VkShaderModuleCreateInfo smCI{};\n");
    c.push_str("    smCI.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO;\n");
    c.push_str("    smCI.codeSize = spv.size() * 4;\n");
    c.push_str("    smCI.pCode = spv.data();\n");
    c.push_str("    VkShaderModule shaderModule;\n");
    c.push_str("    VK_CHECK(vkCreateShaderModule(device, &smCI, nullptr, &shaderModule));\n");
    c.push_str("\n");
    c.push_str("    // Descriptor set layout\n");
    c.push_str("    VkDescriptorSetLayoutBinding bindings[2] = {};\n");
    c.push_str("    bindings[0].binding = 0;\n");
    c.push_str("    bindings[0].descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;\n");
    c.push_str("    bindings[0].descriptorCount = 1;\n");
    c.push_str("    bindings[0].stageFlags = VK_SHADER_STAGE_COMPUTE_BIT;\n");
    c.push_str("    bindings[1].binding = 1;\n");
    c.push_str("    bindings[1].descriptorType = VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER;\n");
    c.push_str("    bindings[1].descriptorCount = 1;\n");
    c.push_str("    bindings[1].stageFlags = VK_SHADER_STAGE_COMPUTE_BIT;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorSetLayoutCreateInfo dslCI{};\n");
    c.push_str("    dslCI.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO;\n");
    c.push_str("    dslCI.bindingCount = 2;\n");
    c.push_str("    dslCI.pBindings = bindings;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorSetLayout dsl;\n");
    c.push_str("    VK_CHECK(vkCreateDescriptorSetLayout(device, &dslCI, nullptr, &dsl));\n");
    c.push_str("\n");
    c.push_str("    // Pipeline layout\n");
    c.push_str("    VkPipelineLayoutCreateInfo plCI{};\n");
    c.push_str("    plCI.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO;\n");
    c.push_str("    plCI.setLayoutCount = 1;\n");
    c.push_str("    plCI.pSetLayouts = &dsl;\n");
    c.push_str("\n");
    c.push_str("    VkPipelineLayout pipelineLayout;\n");
    c.push_str("    VK_CHECK(vkCreatePipelineLayout(device, &plCI, nullptr, &pipelineLayout));\n");
    c.push_str("\n");
    c.push_str("    // Compute pipeline\n");
    c.push_str("    VkComputePipelineCreateInfo cpCI{};\n");
    c.push_str("    cpCI.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO;\n");
    c.push_str("    cpCI.stage.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;\n");
    c.push_str("    cpCI.stage.stage = VK_SHADER_STAGE_COMPUTE_BIT;\n");
    c.push_str("    cpCI.stage.module = shaderModule;\n");
    c.push_str("    cpCI.stage.pName = \"main\";\n");
    c.push_str("    cpCI.layout = pipelineLayout;\n");
    c.push_str("\n");
    c.push_str("    VkPipeline pipeline;\n");
    c.push_str("    VK_CHECK(vkCreateComputePipelines(device, VK_NULL_HANDLE, 1, &cpCI, nullptr, &pipeline));\n");
    c.push_str("\n");
    c.push_str("    // Memory type finder\n");
    c.push_str("    auto findMemType = [&](uint32_t typeBits, VkMemoryPropertyFlags props) -> uint32_t {\n");
    c.push_str("        VkPhysicalDeviceMemoryProperties memProps;\n");
    c.push_str("        vkGetPhysicalDeviceMemoryProperties(physDev, &memProps);\n");
    c.push_str("        for (uint32_t i = 0; i < memProps.memoryTypeCount; i++) {\n");
    c.push_str("            if ((typeBits & (1 << i)) && (memProps.memoryTypes[i].propertyFlags & props) == props)\n");
    c.push_str("                return i;\n");
    c.push_str("        }\n");
    c.push_str("        return UINT32_MAX;\n");
    c.push_str("    };\n");
    c.push_str("\n");
    c.push_str("    // Create SSBO for float data\n");
    c.push_str("    const uint32_t COUNT = 8;\n");
    c.push_str("    float h_data[COUNT] = {1.0f, 2.0f, 3.0f, 4.0f, 5.0f, 6.0f, 7.0f, 8.0f};\n");
    c.push_str("    VkDeviceSize bufSize = COUNT * sizeof(float);\n");
    c.push_str("\n");
    c.push_str("    VkBufferCreateInfo bufCI{};\n");
    c.push_str("    bufCI.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO;\n");
    c.push_str("    bufCI.size = bufSize;\n");
    c.push_str("    bufCI.usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT;\n");
    c.push_str("    bufCI.sharingMode = VK_SHARING_MODE_EXCLUSIVE;\n");
    c.push_str("\n");
    c.push_str("    VkBuffer dataBuf;\n");
    c.push_str("    VK_CHECK(vkCreateBuffer(device, &bufCI, nullptr, &dataBuf));\n");
    c.push_str("\n");
    c.push_str("    VkMemoryRequirements memReq;\n");
    c.push_str("    vkGetBufferMemoryRequirements(device, dataBuf, &memReq);\n");
    c.push_str("\n");
    c.push_str("    VkMemoryAllocateInfo allocInfo{};\n");
    c.push_str("    allocInfo.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO;\n");
    c.push_str("    allocInfo.allocationSize = memReq.size;\n");
    c.push_str("    allocInfo.memoryTypeIndex = findMemType(memReq.memoryTypeBits,\n");
    c.push_str("        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);\n");
    c.push_str("\n");
    c.push_str("    VkDeviceMemory dataMem;\n");
    c.push_str("    VK_CHECK(vkAllocateMemory(device, &allocInfo, nullptr, &dataMem));\n");
    c.push_str("    VK_CHECK(vkBindBufferMemory(device, dataBuf, dataMem, 0));\n");
    c.push_str("\n");
    c.push_str("    void* mapped;\n");
    c.push_str("    VK_CHECK(vkMapMemory(device, dataMem, 0, bufSize, 0, &mapped));\n");
    c.push_str("    memcpy(mapped, h_data, bufSize);\n");
    c.push_str("    vkUnmapMemory(device, dataMem);\n");
    c.push_str("\n");
    c.push_str("    // Create uniform buffer (count)\n");
    c.push_str("    uint32_t count_val = COUNT;\n");
    c.push_str("    VkBufferCreateInfo ubCI{};\n");
    c.push_str("    ubCI.sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO;\n");
    c.push_str("    ubCI.size = sizeof(uint32_t);\n");
    c.push_str("    ubCI.usage = VK_BUFFER_USAGE_UNIFORM_BUFFER_BIT;\n");
    c.push_str("    ubCI.sharingMode = VK_SHARING_MODE_EXCLUSIVE;\n");
    c.push_str("\n");
    c.push_str("    VkBuffer uniformBuf;\n");
    c.push_str("    VK_CHECK(vkCreateBuffer(device, &ubCI, nullptr, &uniformBuf));\n");
    c.push_str("    vkGetBufferMemoryRequirements(device, uniformBuf, &memReq);\n");
    c.push_str("    allocInfo.allocationSize = memReq.size;\n");
    c.push_str("    allocInfo.memoryTypeIndex = findMemType(memReq.memoryTypeBits,\n");
    c.push_str("        VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);\n");
    c.push_str("    VkDeviceMemory uniformMem;\n");
    c.push_str("    VK_CHECK(vkAllocateMemory(device, &allocInfo, nullptr, &uniformMem));\n");
    c.push_str("    VK_CHECK(vkBindBufferMemory(device, uniformBuf, uniformMem, 0));\n");
    c.push_str("    VK_CHECK(vkMapMemory(device, uniformMem, 0, sizeof(uint32_t), 0, &mapped));\n");
    c.push_str("    memcpy(mapped, &count_val, sizeof(uint32_t));\n");
    c.push_str("    vkUnmapMemory(device, uniformMem);\n");
    c.push_str("\n");
    c.push_str("    // Descriptor pool + set\n");
    c.push_str("    VkDescriptorPoolSize poolSizes[2] = {};\n");
    c.push_str("    poolSizes[0].type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;\n");
    c.push_str("    poolSizes[0].descriptorCount = 1;\n");
    c.push_str("    poolSizes[1].type = VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER;\n");
    c.push_str("    poolSizes[1].descriptorCount = 1;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorPoolCreateInfo dpCI{};\n");
    c.push_str("    dpCI.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO;\n");
    c.push_str("    dpCI.maxSets = 1;\n");
    c.push_str("    dpCI.poolSizeCount = 2;\n");
    c.push_str("    dpCI.pPoolSizes = poolSizes;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorPool descPool;\n");
    c.push_str("    VK_CHECK(vkCreateDescriptorPool(device, &dpCI, nullptr, &descPool));\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorSetAllocateInfo dsAI{};\n");
    c.push_str("    dsAI.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;\n");
    c.push_str("    dsAI.descriptorPool = descPool;\n");
    c.push_str("    dsAI.descriptorSetCount = 1;\n");
    c.push_str("    dsAI.pSetLayouts = &dsl;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorSet descSet;\n");
    c.push_str("    VK_CHECK(vkAllocateDescriptorSets(device, &dsAI, &descSet));\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorBufferInfo dbInfo{};\n");
    c.push_str("    dbInfo.buffer = dataBuf;\n");
    c.push_str("    dbInfo.offset = 0;\n");
    c.push_str("    dbInfo.range = bufSize;\n");
    c.push_str("\n");
    c.push_str("    VkDescriptorBufferInfo ubInfo{};\n");
    c.push_str("    ubInfo.buffer = uniformBuf;\n");
    c.push_str("    ubInfo.offset = 0;\n");
    c.push_str("    ubInfo.range = sizeof(uint32_t);\n");
    c.push_str("\n");
    c.push_str("    VkWriteDescriptorSet writes[2] = {};\n");
    c.push_str("    writes[0].sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;\n");
    c.push_str("    writes[0].dstSet = descSet;\n");
    c.push_str("    writes[0].dstBinding = 0;\n");
    c.push_str("    writes[0].descriptorCount = 1;\n");
    c.push_str("    writes[0].descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;\n");
    c.push_str("    writes[0].pBufferInfo = &dbInfo;\n");
    c.push_str("    writes[1].sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;\n");
    c.push_str("    writes[1].dstSet = descSet;\n");
    c.push_str("    writes[1].dstBinding = 1;\n");
    c.push_str("    writes[1].descriptorCount = 1;\n");
    c.push_str("    writes[1].descriptorType = VK_DESCRIPTOR_TYPE_UNIFORM_BUFFER;\n");
    c.push_str("    writes[1].pBufferInfo = &ubInfo;\n");
    c.push_str("    vkUpdateDescriptorSets(device, 2, writes, 0, nullptr);\n");
    c.push_str("\n");
    c.push_str("    // Command pool and buffer\n");
    c.push_str("    VkCommandPoolCreateInfo cpoolCI{};\n");
    c.push_str("    cpoolCI.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;\n");
    c.push_str("    cpoolCI.queueFamilyIndex = computeQF;\n");
    c.push_str("\n");
    c.push_str("    VkCommandPool cmdPool;\n");
    c.push_str("    VK_CHECK(vkCreateCommandPool(device, &cpoolCI, nullptr, &cmdPool));\n");
    c.push_str("\n");
    c.push_str("    VkCommandBufferAllocateInfo cbAI{};\n");
    c.push_str("    cbAI.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;\n");
    c.push_str("    cbAI.commandPool = cmdPool;\n");
    c.push_str("    cbAI.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;\n");
    c.push_str("    cbAI.commandBufferCount = 1;\n");
    c.push_str("\n");
    c.push_str("    VkCommandBuffer cmdBuf;\n");
    c.push_str("    VK_CHECK(vkAllocateCommandBuffers(device, &cbAI, &cmdBuf));\n");
    c.push_str("\n");
    c.push_str("    VkCommandBufferBeginInfo cbBI{};\n");
    c.push_str("    cbBI.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;\n");
    c.push_str("    cbBI.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;\n");
    c.push_str("\n");
    c.push_str("    VK_CHECK(vkBeginCommandBuffer(cmdBuf, &cbBI));\n");
    c.push_str("    vkCmdBindPipeline(cmdBuf, VK_PIPELINE_BIND_POINT_COMPUTE, pipeline);\n");
    c.push_str("    vkCmdBindDescriptorSets(cmdBuf, VK_PIPELINE_BIND_POINT_COMPUTE, pipelineLayout, 0, 1, &descSet, 0, nullptr);\n");
    c.push_str("    vkCmdDispatch(cmdBuf, (COUNT + 255) / 256, 1, 1);\n");
    c.push_str("    VK_CHECK(vkEndCommandBuffer(cmdBuf));\n");
    c.push_str("\n");
    c.push_str("    // Submit and wait\n");
    c.push_str("    VkSubmitInfo submitInfo{};\n");
    c.push_str("    submitInfo.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;\n");
    c.push_str("    submitInfo.commandBufferCount = 1;\n");
    c.push_str("    submitInfo.pCommandBuffers = &cmdBuf;\n");
    c.push_str("\n");
    c.push_str("    VkFenceCreateInfo fenceCI{};\n");
    c.push_str("    fenceCI.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;\n");
    c.push_str("    VkFence fence;\n");
    c.push_str("    VK_CHECK(vkCreateFence(device, &fenceCI, nullptr, &fence));\n");
    c.push_str("    VK_CHECK(vkQueueSubmit(queue, 1, &submitInfo, fence));\n");
    c.push_str("    VK_CHECK(vkWaitForFences(device, 1, &fence, VK_TRUE, UINT64_MAX));\n");
    c.push_str("\n");
    c.push_str("    // Read results\n");
    c.push_str("    VK_CHECK(vkMapMemory(device, dataMem, 0, bufSize, 0, &mapped));\n");
    c.push_str("    memcpy(h_data, mapped, bufSize);\n");
    c.push_str("    vkUnmapMemory(device, dataMem);\n");
    c.push_str("\n");
    c.push_str(&format!("    printf(\"[Salvation] {} -> data:\\n\");\n", kernel_name));
    c.push_str("    for (uint32_t i = 0; i < COUNT; i++) {\n");
    c.push_str("        printf(\"  [%u] = %.1f\\n\", i, h_data[i]);\n");
    c.push_str("    }\n");
    c.push_str("\n");
    c.push_str("    // Cleanup\n");
    c.push_str("    vkDestroyFence(device, fence, nullptr);\n");
    c.push_str("    vkDestroyCommandPool(device, cmdPool, nullptr);\n");
    c.push_str("    vkDestroyDescriptorPool(device, descPool, nullptr);\n");
    c.push_str("    vkDestroyBuffer(device, dataBuf, nullptr);\n");
    c.push_str("    vkFreeMemory(device, dataMem, nullptr);\n");
    c.push_str("    vkDestroyBuffer(device, uniformBuf, nullptr);\n");
    c.push_str("    vkFreeMemory(device, uniformMem, nullptr);\n");
    c.push_str("    vkDestroyPipeline(device, pipeline, nullptr);\n");
    c.push_str("    vkDestroyPipelineLayout(device, pipelineLayout, nullptr);\n");
    c.push_str("    vkDestroyDescriptorSetLayout(device, dsl, nullptr);\n");
    c.push_str("    vkDestroyShaderModule(device, shaderModule, nullptr);\n");
    c.push_str("    vkDestroyDevice(device, nullptr);\n");
    c.push_str("    vkDestroyInstance(instance, nullptr);\n");
    c.push_str("    return 0;\n");
    c.push_str("}\n");

    c
}

fn gen_vulkan_graphics_main(_info: &ShaderInfo) -> String {
    String::from(
        "#include <vulkan/vulkan.h>\n\
         #include <GLFW/glfw3.h>\n\
         #include <cstdio>\n\
         \n\
         int main() {\n\
         \tprintf(\"Graphics mode not fully implemented\\n\");\n\
         \treturn 0;\n\
         }\n",
    )
}
