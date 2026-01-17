FROM amazon/aws-cli:2.15.32
RUN yum install -y jq gzip \
    && yum clean all \
    && rm -rf /var/cache/yum
COPY download.sh /download.sh
RUN chmod +x /download.sh
CMD ["/download.sh"]