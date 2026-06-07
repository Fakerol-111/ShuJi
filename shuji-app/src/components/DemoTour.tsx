import { useState } from "react";
import { Button } from "./ui/Button";

const STORAGE_KEY = "shuji_demo_tour_done";

const STEPS = [
  {
    title: "部门状态栏",
    description:
      "底部状态栏（整个界面最底行）展示所有部门的实时活动状态。点亮的部门表示正在执行任务，您可以随时了解当前进度。",
  },
  {
    title: "文档架阁",
    description:
      "左侧边栏（活动栏右侧）列出了所有产出文档——设计、审查、报告，归档有序。点击即可预览详情。",
  },
  {
    title: "工部修 bug",
    description:
      "工部尚书正在修复 calc.py 中的 bug。您可以在底部「值事」区域看到工部尚书活跃，左侧文档树也会陆续出现新文档。",
  },
  {
    title: "测试验证",
    description:
      "刑部尚书将对修复结果进行测试验证。全部通过后底部状态栏显示「诸司无事」，一条完整的 Demo 流程就完成了。",
  },
];

interface DemoTourProps {
  onClose: () => void;
}

export default function DemoTour({ onClose }: DemoTourProps) {
  const [step, setStep] = useState(0);
  const isLast = step === STEPS.length - 1;

  const handleDone = () => {
    try {
      localStorage.setItem(STORAGE_KEY, "true");
    } catch {
      /* localStorage unavailable */
    }
    onClose();
  };

  const handleSkip = () => {
    handleDone();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-ink-950/60" onClick={handleSkip} />

      {/* Card */}
      <div className="relative bg-surface-elevated border border-fold rounded-2xl shadow-2xl w-full max-w-md mx-4 p-6">
        {/* Step indicator dots */}
        <div className="flex items-center justify-center gap-1.5 mb-4">
          {STEPS.map((_, i) => (
            <div
              key={i}
              className={`w-2 h-2 rounded-full transition-colors ${
                i === step ? "bg-vermillion" : "bg-ink-300"
              }`}
            />
          ))}
        </div>

        {/* Step number */}
        <p className="text-caption text-ink-500 font-medium text-center mb-1">
          {step + 1} / {STEPS.length}
        </p>

        {/* Title */}
        <h2 className="font-display text-display font-bold text-ink-900 text-center mb-2">
          {STEPS[step].title}
        </h2>

        {/* Description */}
        <p className="text-body text-ink-700 text-center leading-relaxed mb-6">
          {STEPS[step].description}
        </p>

        {/* Actions */}
        <div className="flex items-center justify-between">
          <button
            onClick={handleSkip}
            className="text-ui text-ink-500 hover:text-ink-700 px-2 py-1 transition-colors"
          >
            跳过引导
          </button>

          <div className="flex items-center gap-2">
            {step > 0 && (
              <Button variant="secondary" onClick={() => setStep(step - 1)}>
                上一步
              </Button>
            )}
            {isLast ? (
              <Button variant="primary" onClick={handleDone}>
                知道了
              </Button>
            ) : (
              <Button variant="primary" onClick={() => setStep(step + 1)}>
                下一步
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
