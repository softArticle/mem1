# mem1 evaluation (run from evaluation/)
PYTHON ?= python3

DATA ?= dataset/locomo10.json
RESULTS_DIR ?= results
TOP_K ?= 30
# Official LOCOMO from snap-research (GitHub); avoids Google Drive connection issues
LOCOMO_URL ?= https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json

# Number of conversations to include in medium (from start of locomo10.json)
MEDIUM_CONVS ?= 2

.PHONY: add search evals scores sample medium full download-locomo dataset/medium_locomo.json

# Add memories from dataset to mem1-server
add:
	$(PYTHON) run_experiments.py --method add --data_path $(DATA)

# Search + LLM answer; requires OPENAI_API_KEY for answers
search:
	$(PYTHON) run_experiments.py --method search --data_path $(DATA) --output_folder $(RESULTS_DIR) --top_k $(TOP_K)

# Compute BLEU, F1, LLM judge (writes evaluation_metrics.json)
evals:
	$(PYTHON) evals.py --input_file $(RESULTS_DIR)/mem1_results.json --output_file evaluation_metrics.json

# Print mean scores per category and overall
scores:
	$(PYTHON) generate_scores.py evaluation_metrics.json

# Quick run on sample data (1 conversation, 1 QA — very fast, not representative)
sample:
	$(MAKE) add DATA=dataset/sample_locomo.json
	$(MAKE) search DATA=dataset/sample_locomo.json RESULTS_DIR=results
	$(MAKE) evals
	$(MAKE) scores

# Medium: first MEDIUM_CONVS conversations from locomo10 (~300–500 QAs). Build with: make dataset/medium_locomo.json
dataset/medium_locomo.json: download-locomo
	$(PYTHON) -c "import json; d=json.load(open('dataset/locomo10.json')); n=int('$(MEDIUM_CONVS)'); json.dump(d[:n], open('dataset/medium_locomo.json','w'), indent=2)"
	@echo "dataset/medium_locomo.json ready (first $(MEDIUM_CONVS) convs)"

# Run eval on medium subset (bigger than sample, faster than full)
medium: dataset/medium_locomo.json
	$(MAKE) add DATA=dataset/medium_locomo.json
	$(MAKE) search DATA=dataset/medium_locomo.json RESULTS_DIR=results
	$(MAKE) evals
	$(MAKE) scores

# Download locomo10.json from GitHub (snap-research/locomo). Idempotent: skips if file exists.
download-locomo:
	@mkdir -p dataset && \
	if [ ! -f dataset/locomo10.json ]; then \
	  echo "Downloading LOCOMO dataset from GitHub..."; \
	  curl -fSL "$(LOCOMO_URL)" -o dataset/locomo10.json || (echo "Download failed; try: curl -fSL $(LOCOMO_URL) -o dataset/locomo10.json" && exit 1); \
	fi; \
	test -f dataset/locomo10.json && echo "dataset/locomo10.json ready"

# Full LOCOMO: download (if needed) + add + search + evals + scores
full: download-locomo
	$(MAKE) add
	$(MAKE) search
	$(MAKE) evals
	$(MAKE) scores
