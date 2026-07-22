// 待办事项管理应用
// 功能：添加、编辑、删除、完成标记、筛选、本地存储

class TodoApp {
  constructor() {
    this.todos = [];
    this.currentFilter = "all"; // all, active, completed
    this.init();
  }

  // 初始化应用
  init() {
    this.loadFromLocalStorage();
    this.render();
    this.bindEvents();
  }

  // 从本地存储加载数据
  loadFromLocalStorage() {
    const stored = localStorage.getItem("todos");
    if (stored) {
      this.todos = JSON.parse(stored);
    } else {
      // 添加示例数据
      this.todos = [
        { id: 1, text: "学习JavaScript", completed: false },
        { id: 2, text: "完成项目报告", completed: false },
        { id: 3, text: "买菜", completed: true },
      ];
    }
  }

  // 保存到本地存储
  saveToLocalStorage() {
    localStorage.setItem("todos", JSON.stringify(this.todos));
  }

  // 生成唯一ID
  generateId() {
    return Date.now() + Math.random();
  }

  // 添加待办事项
  addTodo(text) {
    if (!text.trim()) return false;
    const todo = {
      id: this.generateId(),
      text: text.trim(),
      completed: false,
      createdAt: new Date().toISOString(),
    };
    this.todos.push(todo);
    this.saveToLocalStorage();
    this.render();
    return true;
  }

  // 删除待办事项
  deleteTodo(id) {
    this.todos = this.todos.filter((todo) => todo.id !== id);
    this.saveToLocalStorage();
    this.render();
  }

  // 切换完成状态
  toggleTodo(id) {
    const todo = this.todos.find((todo) => todo.id === id);
    if (todo) {
      todo.completed = !todo.completed;
      this.saveToLocalStorage();
      this.render();
    }
  }

  // 编辑待办事项
  editTodo(id, newText) {
    if (!newText.trim()) return false;
    const todo = this.todos.find((todo) => todo.id === id);
    if (todo) {
      todo.text = newText.trim();
      this.saveToLocalStorage();
      this.render();
      return true;
    }
    return false;
  }

  // 获取筛选后的待办事项
  getFilteredTodos() {
    if (this.currentFilter === "active") {
      return this.todos.filter((todo) => !todo.completed);
    } else if (this.currentFilter === "completed") {
      return this.todos.filter((todo) => todo.completed);
    }
    return this.todos;
  }

  // 获取统计数据
  getStats() {
    const total = this.todos.length;
    const completed = this.todos.filter((todo) => todo.completed).length;
    const active = total - completed;
    return { total, completed, active };
  }

  // 清除所有已完成的事项
  clearCompleted() {
    this.todos = this.todos.filter((todo) => !todo.completed);
    this.saveToLocalStorage();
    this.render();
  }

  // 绑定DOM事件
  bindEvents() {
    // 添加待办事项
    const addBtn = document.getElementById("addTodoBtn");
    const todoInput = document.getElementById("todoInput");

    if (addBtn && todoInput) {
      addBtn.addEventListener("click", () => {
        this.addTodo(todoInput.value);
        todoInput.value = "";
        todoInput.focus();
      });

      todoInput.addEventListener("keypress", (e) => {
        if (e.key === "Enter") {
          this.addTodo(todoInput.value);
          todoInput.value = "";
        }
      });
    }

    // 筛选按钮事件
    const filterBtns = document.querySelectorAll(".filter-btn");
    filterBtns.forEach((btn) => {
      btn.addEventListener("click", (e) => {
        this.currentFilter = e.target.dataset.filter;
        this.render();
      });
    });

    // 清除已完成按钮
    const clearBtn = document.getElementById("clearCompleted");
    if (clearBtn) {
      clearBtn.addEventListener("click", () => this.clearCompleted());
    }
  }

  // 渲染待办事项列表
  render() {
    const todoList = document.getElementById("todoList");
    if (!todoList) return;

    const filteredTodos = this.getFilteredTodos();
    const stats = this.getStats();

    if (filteredTodos.length === 0) {
      todoList.innerHTML = '<div class="empty-state">暂无待办事项</div>';
    } else {
      todoList.innerHTML = filteredTodos
        .map(
          (todo) => `
        <div class="todo-item ${todo.completed ? "completed" : ""}" data-id="${todo.id}">
          <input type="checkbox" class="todo-checkbox" ${todo.completed ? "checked" : ""}>
          <span class="todo-text">${this.escapeHtml(todo.text)}</span>
          <button class="edit-btn">编辑</button>
          <button class="delete-btn">删除</button>
        </div>
      `,
        )
        .join("");

      // 绑定待办事项事件
      todoList.querySelectorAll(".todo-item").forEach((item) => {
        const id = parseFloat(item.dataset.id);

        const checkbox = item.querySelector(".todo-checkbox");
        checkbox.addEventListener("change", () => this.toggleTodo(id));

        const deleteBtn = item.querySelector(".delete-btn");
        deleteBtn.addEventListener("click", () => this.deleteTodo(id));

        const editBtn = item.querySelector(".edit-btn");
        editBtn.addEventListener("click", () => {
          const newText = prompt(
            "编辑待办事项:",
            this.todos.find((t) => t.id === id).text,
          );
          if (newText) this.editTodo(id, newText);
        });
      });
    }

    // 更新统计信息
    this.updateStats(stats);
    this.updateFilterButtons();
  }

  // 更新统计信息显示
  updateStats(stats) {
    const statsEl = document.getElementById("stats");
    if (statsEl) {
      statsEl.innerHTML = `
        总计: ${stats.total} |
        已完成: ${stats.completed} |
        未完成: ${stats.active}
      `;
    }
  }

  // 更新筛选按钮状态
  updateFilterButtons() {
    const filterBtns = document.querySelectorAll(".filter-btn");
    filterBtns.forEach((btn) => {
      if (btn.dataset.filter === this.currentFilter) {
        btn.classList.add("active");
      } else {
        btn.classList.remove("active");
      }
    });
  }

  // 防止XSS攻击
  escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }
}

// 页面加载完成后初始化应用
document.addEventListener("DOMContentLoaded", () => {
  new TodoApp();
});
