package main

import (
    "encoding/json"
    "fmt"
    "net/http"
    "strconv"
    "sync"
    "time"
)

// Task 表示一个计算任务
type Task struct {
    ID        int       `json:"id"`
    N         int       `json:"n"`
    Status    string    `json:"status"` // pending, running, completed, failed
    Result    uint64    `json:"result,omitempty"`
    CreatedAt time.Time `json:"created_at"`
    UpdatedAt time.Time `json:"updated_at"`
}

var (
    tasks   = make(map[int]*Task) // 任务存储
    mu      sync.RWMutex          // 保护 tasks 的读写锁
    idSeq   = 0                   // 任务ID自增序列
    taskCh  = make(chan int, 100) // 任务ID通道，worker从中获取任务
    numWorkers = 5                // Worker 数量
)

// fibonacci 计算第 n 个斐波那契数（迭代法，避免递归栈溢出）
func fibonacci(n int) uint64 {
    if n < 0 {
        return 0
    }
    if n == 0 {
        return 0
    }
    if n == 1 {
        return 1
    }
    var a, b uint64 = 0, 1
    for i := 2; i <= n; i++ {
        a, b = b, a+b
    }
    return b
}

// worker 后台工作协程，从通道中读取任务ID并执行计算
func worker(workerID int) {
    for taskID := range taskCh {
        // 获取任务并更新状态为 running
        mu.Lock()
        t, ok := tasks[taskID]
        if !ok {
            mu.Unlock()
            continue
        }
        t.Status = "running"
        t.UpdatedAt = time.Now()
        mu.Unlock()

        // 执行计算
        result := fibonacci(t.N)

        // 更新任务结果
        mu.Lock()
        if t, ok := tasks[taskID]; ok {
            t.Result = result
            t.Status = "completed"
            t.UpdatedAt = time.Now()
        }
        mu.Unlock()
        fmt.Printf("Worker %d finished task %d (fib(%d)=%d)\n", workerID, taskID, t.N, result)
    }
}

// handleListTasks 返回所有任务的列表
func handleListTasks(w http.ResponseWriter, r *http.Request) {
    mu.RLock()
    defer mu.RUnlock()
    list := make([]*Task, 0, len(tasks))
    for _, t := range tasks {
        list = append(list, t)
    }
    respondJSON(w, http.StatusOK, list)
}

// handleCreateTask 创建新任务，放入队列
func handleCreateTask(w http.ResponseWriter, r *http.Request) {
    var req struct {
        N int `json:"n"`
    }
    if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
        respondError(w, http.StatusBadRequest, "Invalid JSON")
        return
    }
    if req.N < 0 {
        respondError(w, http.StatusBadRequest, "N must be non-negative")
        return
    }

    mu.Lock()
    idSeq++
    taskID := idSeq
    task := &Task{
        ID:        taskID,
        N:         req.N,
        Status:    "pending",
        CreatedAt: time.Now(),
        UpdatedAt: time.Now(),
    }
    tasks[taskID] = task
    mu.Unlock()

    // 将任务ID放入通道，等待worker处理
    taskCh <- taskID

    respondJSON(w, http.StatusCreated, task)
}

// handleGetTask 返回单个任务的详细信息
func handleGetTask(w http.ResponseWriter, r *http.Request) {
    idStr := r.URL.Path[len("/tasks/"):]
    id, err := strconv.Atoi(idStr)
    if err != nil {
        respondError(w, http.StatusBadRequest, "Invalid task ID")
        return
    }

    mu.RLock()
    task, ok := tasks[id]
    mu.RUnlock()
    if !ok {
        respondError(w, http.StatusNotFound, "Task not found")
        return
    }
    respondJSON(w, http.StatusOK, task)
}

// respondJSON 以JSON格式响应数据
func respondJSON(w http.ResponseWriter, status int, data interface{}) {
    w.Header().Set("Content-Type", "application/json")
    w.WriteHeader(status)
    json.NewEncoder(w).Encode(data)
}

// respondError 返回错误信息
func respondError(w http.ResponseWriter, status int, message string) {
    respondJSON(w, status, map[string]string{"error": message})
}

func main() {
    // 启动 worker 池
    for i := 0; i < numWorkers; i++ {
        go worker(i + 1)
    }

    // 注册路由
    http.HandleFunc("/tasks", func(w http.ResponseWriter, r *http.Request) {
        switch r.Method {
        case http.MethodGet:
            handleListTasks(w, r)
        case http.MethodPost:
            handleCreateTask(w, r)
        default:
            respondError(w, http.StatusMethodNotAllowed, "Method not allowed")
        }
    })
    http.HandleFunc("/tasks/", handleGetTask)

    fmt.Println("Server running on :8080")
    fmt.Println("Endpoints:")
    fmt.Println("  GET    /tasks       - List all tasks")
    fmt.Println("  POST   /tasks       - Create new task (JSON: {\"n\": value})")
    fmt.Println("  GET    /tasks/{id}  - Get task by ID")
    if err := http.ListenAndServe(":8080", nil); err != nil {
        fmt.Printf("Server failed: %v\n", err)
    }
}
