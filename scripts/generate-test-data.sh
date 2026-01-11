#!/bin/bash
# Generate minimal test data files for htsget integration tests
# Requires: samtools, bcftools, bgzip, tabix

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${SCRIPT_DIR}/../tests/data"

mkdir -p "${DATA_DIR}"

echo "Generating test data in ${DATA_DIR}..."

# Create a minimal SAM file with a few reads
cat > "${DATA_DIR}/sample.sam" << 'EOF'
@HD	VN:1.6	SO:coordinate
@SQ	SN:chr1	LN:1000000
@SQ	SN:chr2	LN:500000
@RG	ID:sample	SM:sample
read1	0	chr1	100	60	50M	*	0	0	AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA	*	RG:Z:sample
read2	0	chr1	200	60	50M	*	0	0	GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG	*	RG:Z:sample
read3	0	chr1	300	60	50M	*	0	0	CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC	*	RG:Z:sample
read4	0	chr2	100	60	50M	*	0	0	TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT	*	RG:Z:sample
EOF

# Convert to BAM and index
echo "Creating BAM file..."
samtools view -bS "${DATA_DIR}/sample.sam" > "${DATA_DIR}/sample.bam"
samtools index "${DATA_DIR}/sample.bam"

# Create CRAM (requires reference, but we can use embedded reference)
echo "Creating CRAM file..."
samtools view -C -o "${DATA_DIR}/sample.cram" "${DATA_DIR}/sample.sam"
samtools index "${DATA_DIR}/sample.cram"

# Create a minimal VCF
echo "Creating VCF file..."
cat > "${DATA_DIR}/sample.vcf" << 'EOF'
##fileformat=VCFv4.2
##contig=<ID=chr1,length=1000000>
##contig=<ID=chr2,length=500000>
##INFO=<ID=DP,Number=1,Type=Integer,Description="Depth">
##FORMAT=<ID=GT,Number=1,Type=String,Description="Genotype">
#CHROM	POS	ID	REF	ALT	QUAL	FILTER	INFO	FORMAT	sample
chr1	100	.	A	G	50	PASS	DP=30	GT	0/1
chr1	200	.	C	T	60	PASS	DP=40	GT	1/1
chr2	150	.	G	A	45	PASS	DP=25	GT	0/1
EOF

# Compress and index VCF
bgzip -f "${DATA_DIR}/sample.vcf"
tabix -p vcf "${DATA_DIR}/sample.vcf.gz"

# Create BCF (if bcftools is available)
if command -v bcftools &> /dev/null; then
    echo "Creating BCF file..."
    bcftools view -Ob -o "${DATA_DIR}/sample.bcf" "${DATA_DIR}/sample.vcf.gz"
    bcftools index "${DATA_DIR}/sample.bcf"
else
    echo "Skipping BCF creation (bcftools not found)"
fi

# Create a minimal FASTA
echo "Creating FASTA file..."
cat > "${DATA_DIR}/sample.fa" << 'EOF'
>chr1
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG
CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
>chr2
TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
EOF

# Index FASTA
samtools faidx "${DATA_DIR}/sample.fa"

# Create a minimal FASTQ (gzipped)
echo "Creating FASTQ file..."
cat > "${DATA_DIR}/sample.fq" << 'EOF'
@read1
AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
+
IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII
@read2
GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG
+
IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII
@read3
CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
+
IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII
EOF

gzip -f "${DATA_DIR}/sample.fq"

# Clean up intermediate files
rm -f "${DATA_DIR}/sample.sam"

echo "Test data generation complete!"
echo "Files created:"
ls -la "${DATA_DIR}/"
