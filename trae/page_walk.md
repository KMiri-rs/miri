# `page_walk` 对话记录

---

## 一、函数定义

**用户**：解释 `page_walk` 的查找物理地址的逻辑

**回答**：

`page_walk` 实现了经典的 **4 级页表遍历算法**，将虚拟地址（VA）转换为物理地址（PA）。

```rust
pub fn page_walk(&self, vaddr: usize) -> Option<usize> {
    let mut current_paddr = self.root_paddr;
    let mut current_level = NR_LEVELS;

    while current_level >= 1 {
        let index = Self::pte_index(vaddr, current_level);

        let page_table_entry = unsafe {
            let pte_paddr = (current_paddr as *const usize).add(index) as usize;
            *(super::paddr_to_mem(pte_paddr) as *const usize)
        };

        const PTE_MASK: usize = 0xF_FFFF_FFFF_F000;
        current_paddr = page_table_entry & PTE_MASK;
        current_level -= 1;

        if page_table_entry & Self::HUGE_BIT_MASK > 0 {
            break;
        }
    }

    let page_offset = vaddr & ((super::page_size() << (current_level * Self::PTE_INDEX_BITS)) - 1);
    Some(current_paddr + page_offset)
}
```

**位置**：[page_table.rs#L95-119](file:///home/zjp/KMiri/kmiri/src/mirch/page_table.rs#L95-L119)

---

## 二、页表结构

**用户**：`0x1FF` 有什么说法吗

**回答**：

`0x1FF` 是一个**9位全1的二进制掩码**：

```
0x1FF = 511 (十进制) = 0b111111111 (二进制)
```

从代码中可以看到：

```rust
impl PageTable {
    const PTE_PER_PAGE: usize = page_size() / PTE_SIZE;  // 4096 / 8 = 512
    const LEVEL_MASK: usize = Self::PTE_PER_PAGE - 1;    // 512 - 1 = 511 = 0x1FF
}
```

每个页表页可以容纳 **512 个 PTE**，因此索引范围是 `0 ~ 511`，需要 9 位来表示。

---

## 三、页偏移计算

**用户**：解释 `/home/zjp/KMiri/kmiri/src/mirch/page_table.rs#L116-117`

**回答**：

这段代码根据**遍历停止的层级**，动态计算虚拟地址在最终物理页中的**偏移量**。

```rust
let page_offset = vaddr & ((super::page_size() << (current_level * Self::PTE_INDEX_BITS)) - 1);
```

**三种场景**：

1. **标准 4KB 页（`current_level = 0`）**：
   ```rust
   page_offset = vaddr & ((4096 << 0) - 1) = vaddr & 0xFFF  // 取低12位
   ```

2. **2MB 大页（`current_level = 1`）**：
   ```rust
   page_offset = vaddr & ((4096 << 9) - 1) = vaddr & 0x1FFFFF  // 取低21位
   ```

3. **1GB 大页（`current_level = 2`）**：
   ```rust
   page_offset = vaddr & ((4096 << 18) - 1) = vaddr & 0x3FFFFFFF  // 取低30位
   ```

---

## 四、公式来源

**用户**：这样的公式需要如何得到？自己总结吗？还是现成的算法

**回答**：

这个公式并非凭空创造，而是来自 **x86-64 硬件架构规范**，是计算机体系结构中的经典知识。

**推导过程**：

1. **确定虚拟地址布局**：
   ```
   48-bit VA = [L4索引(9位)][L3索引(9位)][L2索引(9位)][L1索引(9位)][页内偏移(12位)]
   ```

2. **计算大页的偏移位数**：
   - 当在第 `n` 级停止时，跳过了 `n` 个层级
   - 总偏移位数 = 基础页偏移(12位) + 跳过层级的索引位数(n × 9位)

3. **转换为掩码**：
   ```
   偏移位数 = 12 + n × 9
   偏移掩码 = (1 << 偏移位数) - 1 = (4096 << (n × 9)) - 1
   ```

---

## 五、公式多样性

**用户**：难道就只有这一种公式计算吗？

**回答**：

当然不是！计算页偏移量有**多种方法**：

### 方法一：位掩码法（当前实现）
```rust
let page_offset = vaddr & ((page_size() << (current_level * PTE_INDEX_BITS)) - 1);
```

### 方法二：位移法
```rust
let shift_amount = page_size().ilog2() as usize + current_level * PTE_INDEX_BITS;
let page_offset = vaddr - (vaddr >> shift_amount << shift_amount);
```

### 方法三：查表法
```rust
const PAGE_MASKS: [usize; 4] = [0xFFF, 0x1FFFFF, 0x3FFFFFFF, 0x7FFFFFFFFF];
let page_offset = vaddr & PAGE_MASKS[current_level];
```

### 方法四：条件分支法
```rust
let page_offset = match current_level {
    0 => vaddr & 0xFFF,
    1 => vaddr & 0x1FFFFF,
    2 => vaddr & 0x3FFFFFFF,
    3 => vaddr & 0x7FFFFFFFFF,
    _ => unreachable!(),
};
```

---

## 六、查表法 vs 位掩码法性能对比

**用户**：查表法和位掩码法哪个高效

**回答**：

| 维度 | 查表法 | 位掩码法 |
|------|--------|----------|
| **指令数量** | 2条 | 4条 |
| **内存访问** | 1次（数组读取） | 0次 |
| **缓存命中** | ~2 周期 | ~3-4 周期 |
| **缓存未命中** | ~100 周期 | ~3-4 周期 |

**结论**：
- 缓存命中时，查表法略快
- 缓存未命中时，位掩码法快得多
- 在页表遍历这种热路径中，由于数组大概率在 L1 缓存中，查表法略优

---

## 七、PTE 有效性检查

**用户**：你建议的增加 PTE 有效性检查有什么理由吗

**回答**：

增加 PTE 有效性检查有以下关键理由：

1. **符合真实硬件行为**：真实 CPU 会检查 PTE 的 P 位、权限位等
2. **安全性**：防止无效地址访问和权限绕过
3. **调试便利**：更早发现问题，错误定位更准确
4. **API 契约**：`page_walk` 返回 `Option`，应该真正返回 `None`

```rust
// 应该增加的检查
if page_table_entry == 0 {
    return None;  // PTE 不存在
}

const VALID_BIT: usize = 1 << 0;
if (page_table_entry & VALID_BIT) == 0 {
    return None;  // PTE 未标记为有效
}
```

---

## 八、标志位设置责任

**用户**：如果这是软件传递的页表，那么 VALID_BIT 需要在构造时就自己设置吗

**回答**：

是的！这涉及到硬件和软件的分工：

### 责任划分

| 角色 | 责任 |
|------|------|
| **内核代码** | 创建和填充页表，设置所有必要的标志位（包括 VALID_BIT） |
| **KMiri** | 验证页表的正确性，模拟 CPU 的页表遍历行为 |

当前 KMiri 的 `init_boot_pt()` 中没有设置 VALID_BIT，这是一个需要修复的问题。

---

## 九、完整页表规范

**用户**：如果需要符合页表规范，那么读写位等其他信息也应该正确实现吧

**回答**：

完全正确！x86-64 页表项有多个标志位：

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Bit 63: NX        │ Bit 52-62: Reserved    │ Bit 51-12: PFN         │
├─────────────────────────────────────────────────────────────────────────┤
│ Bit 11-9: PAT/G   │ Bit 8: Global         │ Bit 7: PAT             │
├─────────────────────────────────────────────────────────────────────────┤
│ Bit 6: Dirty      │ Bit 5: Accessed       │ Bit 4: PCD             │
├─────────────────────────────────────────────────────────────────────────┤
│ Bit 3: PWT        │ Bit 2: U/S           │ Bit 1: R/W             │
├─────────────────────────────────────────────────────────────────────────┤
│ Bit 0: P (Present)│                                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

### 当前实现状态

| 标志位 | 位位置 | 实现状态 |
|--------|--------|----------|
| P (Present) | Bit 0 | ❌ 未检查 |
| R/W (Read/Write) | Bit 1 | ❌ 未检查 |
| U/S (User/Supervisor) | Bit 2 | ❌ 未检查 |
| A (Accessed) | Bit 5 | ❌ 未更新 |
| D (Dirty) | Bit 6 | ❌ 未更新 |
| NX (No Execute) | Bit 63 | ❌ 未检查 |

---

## 十、真实机器上的页表工作流程

**用户**：对于运行在真实机器上的内核，内核设置的页表项需要这些标志位吗？还是说这些标志位是机器设置的

**回答**：

这是硬件和软件的**协作**：

### 内核的职责（软件）
- 设置 P、R/W、U/S、NX 等标志位
- 根据 A/D 位进行页面置换决策

### CPU 的职责（硬件）
- 验证 P、R/W、U/S、NX 等标志位
- 自动更新 A/D 位（访问/写入时）
- 违反权限时触发页错误异常

**总结**：内核设置标志位，CPU 验证和更新标志位。

---

## 十一、KMiri 的实现建议

**用户**：既然内核负责设置标志位，CPU 验证标志位，那么 KMiri 也应该进行标志位校验，并适时更新 A/D 位

**回答**：

您的观点完全正确！KMiri 作为内核模拟器，确实应该实现完整的页表标志位校验和自动更新机制。

### 实现优先级

| 优先级 | 功能 | 理由 |
|--------|------|------|
| **P0** | P 位检查 | 最基本的有效性检查 |
| **P1** | R/W 位检查 | 写保护和 COW 依赖 |
| **P2** | A/D 位自动更新 | 页面置换算法依赖 |
| **P3** | U/S 位检查 | 用户/内核隔离 |
| **P4** | NX 位检查 | 执行保护 |

---

**创建时间**：2026-05-21
