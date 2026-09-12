import vl_convert as vlc

vlc.configure(
    num_workers=2,
    max_v8_heap_size_mb=512,
    max_v8_execution_time_secs=10,
    gc_after_conversion=True,
)
vlc.warm_up_workers()

for worker in vlc.get_worker_memory_usage():
    print(worker["worker_index"], worker["used_heap_size"])
