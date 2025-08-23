const ResizeableTableHeader = (props: { title: string }) => {
    const { title } = props;

    return (
        <th>
            <div className="resize-x">{title}</div>
        </th>
    );
};

export default ResizeableTableHeader;
