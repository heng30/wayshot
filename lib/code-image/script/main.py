import json
import os

class Student:
    """学生类"""
    def __init__(self, student_id, name, scores=None):
        self.student_id = student_id
        self.name = name
        self.scores = scores if scores is not None else {'math': 0, 'english': 0, 'python': 0}

    def average(self):
        """计算平均分"""
        return sum(self.scores.values()) / len(self.scores)

    def total(self):
        """计算总分"""
        return sum(self.scores.values())

    def to_dict(self):
        """转换为字典，用于JSON存储"""
        return {'student_id': self.student_id, 'name': self.name, 'scores': self.scores}

    @staticmethod
    def from_dict(data):
        """从字典创建Student对象"""
        return Student(data['student_id'], data['name'], data['scores'])

    def __str__(self):
        return f"{self.student_id}\t{self.name}\t数学:{self.scores['math']}\t英语:{self.scores['english']}\tPython:{self.scores['python']}\t总分:{self.total()}\t均分:{self.average():.1f}"


class StudentManager:
    """学生成绩管理类"""
    def __init__(self, filename='students.json'):
        self.filename = filename
        self.students = {}  # 键为学生ID，值为Student对象
        self.load_data()

    def add_student(self, student_id, name, math=0, english=0, python=0):
        """添加学生"""
        if student_id in self.students:
            print(f"学生ID {student_id} 已存在！")
            return False
        scores = {'math': math, 'english': english, 'python': python}
        self.students[student_id] = Student(student_id, name, scores)
        print(f"学生 {name}({student_id}) 添加成功！")
        self.save_data()
        return True

    def remove_student(self, student_id):
        """删除学生"""
        if student_id in self.students:
            name = self.students[student_id].name
            del self.students[student_id]
            print(f"学生 {name}({student_id}) 删除成功！")
            self.save_data()
            return True
        else:
            print(f"未找到学生ID: {student_id}")
            return False

    def update_scores(self, student_id, math=None, english=None, python=None):
        """更新学生成绩"""
        if student_id not in self.students:
            print(f"未找到学生ID: {student_id}")
            return False
        student = self.students[student_id]
        if math is not None:
            student.scores['math'] = math
        if english is not None:
            student.scores['english'] = english
        if python is not None:
            student.scores['python'] = python
        print(f"学生 {student.name}({student_id}) 成绩更新成功！")
        self.save_data()
        return True

    def query_student(self, student_id):
        """查询单个学生"""
        if student_id in self.students:
            print(self.students[student_id])
            return self.students[student_id]
        else:
            print(f"未找到学生ID: {student_id}")
            return None

    def list_all(self):
        """列出所有学生"""
        if not self.students:
            print("暂无学生数据。")
            return
        print("\n=== 所有学生成绩列表 ===")
        print("学号\t姓名\t数学\t英语\tPython\t总分\t均分")
        for student in self.students.values():
            print(student)
        print(f"共 {len(self.students)} 名学生。\n")

    def statistics(self):
        """统计信息"""
        if not self.students:
            print("暂无学生数据。")
            return
        totals = [s.total() for s in self.students.values()]
        averages = [s.average() for s in self.students.values()]
        print("\n=== 统计信息 ===")
        print(f"学生总数: {len(self.students)}")
        print(f"总分最高: {max(totals)}")
        print(f"总分最低: {min(totals)}")
        print(f"总分平均: {sum(totals)/len(totals):.1f}")
        print(f"均分最高: {max(averages):.1f}")
        print(f"均分最低: {min(averages):.1f}")
        print(f"全班均分: {sum(averages)/len(averages):.1f}\n")

    def save_data(self):
        """保存数据到JSON文件"""
        data = [s.to_dict() for s in self.students.values()]
        with open(self.filename, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    def load_data(self):
        """从JSON文件加载数据"""
        if not os.path.exists(self.filename):
            return
        with open(self.filename, 'r', encoding='utf-8') as f:
            data = json.load(f)
        self.students = {}
        for item in data:
            s = Student.from_dict(item)
            self.students[s.student_id] = s


def main():
    """主菜单交互"""
    manager = StudentManager()

    while True:
        print("\n====== 学生成绩管理系统 ======")
        print("1. 添加学生")
        print("2. 删除学生")
        print("3. 修改成绩")
        print("4. 查询学生")
        print("5. 显示所有学生")
        print("6. 统计信息")
        print("7. 退出系统")

        choice = input("请选择操作(1-7): ").strip()

        if choice == '1':
            sid = input("学号: ").strip()
            name = input("姓名: ").strip()
            try:
                math = float(input("数学成绩: "))
                english = float(input("英语成绩: "))
                python_score = float(input("Python成绩: "))
                manager.add_student(sid, name, math, english, python_score)
            except ValueError:
                print("成绩请输入数字！")

        elif choice == '2':
            sid = input("请输入要删除的学生学号: ").strip()
            manager.remove_student(sid)

        elif choice == '3':
            sid = input("请输入要修改成绩的学生学号: ").strip()
            print("直接回车表示不修改该科成绩")
            math = input("数学成绩: ").strip()
            english = input("英语成绩: ").strip()
            python_score = input("Python成绩: ").strip()
            math_v = float(math) if math else None
            eng_v = float(english) if english else None
            py_v = float(python_score) if python_score else None
            manager.update_scores(sid, math_v, eng_v, py_v)

        elif choice == '4':
            sid = input("请输入要查询的学生学号: ").strip()
            manager.query_student(sid)

        elif choice == '5':
            manager.list_all()

        elif choice == '6':
            manager.statistics()

        elif choice == '7':
            print("感谢使用，再见！")
            break

        else:
            print("无效输入，请重新选择。")


if __name__ == "__main__":
    main()
