// 导入结果面板文案（REQ-0014）。由该特性的实现者维护。
// key 以 report. 开头。
export const enDiscover: Record<string, string> = {
  'report.title': 'Import result',
  'report.summary': '{added} added, {skipped} skipped, {failed} failed',
  'report.details': 'Details',
  'report.empty': 'No items were imported.',
  'report.copy': 'Copy result',
  'report.copyHeader': 'Import result: {added} added, {skipped} skipped, {failed} failed',
  'report.copied': 'Copied to clipboard',
  'report.copyFailed': 'Clipboard unavailable — text selected, press Ctrl+C to copy',
  'report.state.added': 'Added',
  'report.state.skipped': 'Skipped',
  'report.state.failed': 'Failed',
  'report.done': 'Done',

  // REQ-0041 · AI classification of discover results.
  'discover.classify': 'AI classify',
  'discover.classifying': 'Classifying…',
  'discover.classifyHint': 'AI proposes categories; import keeps them',
  'discover.groupMode': 'Group by',
  'discover.groupCategory': 'Category',
  'discover.groupSource': 'Source folder',

  // Progressive disclosure: short consequence + details behind the info icon.
  'discover.modeInfo': 'Import mode help',
  'discover.modeShort.link': 'Links only; originals stay put',
  'discover.modeShort.move': 'Move into the apps folder; originals are removed',
  'discover.modeShort.register': 'Records the path only; files stay in place',
  'discover.modeDetail.link': 'Folders become directory junctions and single files become hard links. Cross-volume files cannot be hard-linked — use Move or Register instead.',
  'discover.modeDetail.move': 'Copies the program into the apps folder, verifies it, then removes the original location. This is not a migration backup; use Settings → Migrate for a self-contained package.',
  'discover.modeDetail.register': 'Keeps the program where it is and only stores the path. Best for very large apps or software that must stay in place. For a full relocation use Settings → Migrate.'
}

export const zhDiscover: Record<string, string> = {
  'report.title': '导入结果',
  'report.summary': '新增 {added}，跳过 {skipped}，失败 {failed}',
  'report.details': '明细',
  'report.empty': '没有导入任何项目。',
  'report.copy': '复制结果',
  'report.copyHeader': '导入结果：新增 {added}，跳过 {skipped}，失败 {failed}',
  'report.copied': '已复制到剪贴板',
  'report.copyFailed': '剪贴板不可用，已选中文本，请按 Ctrl+C 手动复制',
  'report.state.added': '已新增',
  'report.state.skipped': '跳过',
  'report.state.failed': '失败',
  'report.done': '完成',

  // REQ-0041 · 发现结果的 AI 分类。
  'discover.classify': '智能分类',
  'discover.classifying': '分类中…',
  'discover.classifyHint': 'AI 给出分类建议，导入时一并保存',
  'discover.groupMode': '分组方式',
  'discover.groupCategory': '按分类',
  'discover.groupSource': '按来源目录',

  // 渐进披露：短摘要说明当前选择的后果，细节放在信息图标里。
  'discover.modeInfo': '导入方式说明',
  'discover.modeShort.link': '原文件不动，只建立链接',
  'discover.modeShort.move': '搬到应用目录，原位置删除',
  'discover.modeShort.register': '只记录原路径，不移动文件',
  'discover.modeDetail.link': '文件夹会创建目录联接，单文件使用硬链接；跨盘文件无法建立硬链接，请改用「移动」或「仅登记」。',
  'discover.modeDetail.move': '把程序复制到应用目录并校验，然后删除原位置。这不是迁移备份；需要自包含迁移包请用「设置 → 迁移」。',
  'discover.modeDetail.register': '程序留在原处，只保存路径。适合体积很大或必须固定位置的软件；整体搬家请用「设置 → 迁移」。'
}
